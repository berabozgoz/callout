//! Server certificate: self-signed generation, SHA-256 fingerprint, `callout://`
//! invite link and the client-side "trust on first use" (TOFU) verifier.
//!
//! The foundation of the self-host model: no domain name or CA required. The server
//! carries its fingerprint in the invite link, and the client accepts the certificate
//! only if it matches that fingerprint.

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use sha2::{Digest, Sha256};
use std::fmt;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum CertError {
    #[error("failed to generate certificate: {0}")]
    Generate(#[from] rcgen::Error),
    #[error("certificate file: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid invite link: {0}")]
    BadInvite(String),
}

/// SHA-256 digest of the certificate's DER encoding.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    pub fn of(cert: &CertificateDer<'_>) -> Self {
        Self(Sha256::digest(cert.as_ref()).into())
    }

    /// Short form used in the invite link: 64 hex characters.
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, CertError> {
        let bytes = hex::decode(s.trim()).map_err(|e| CertError::BadInvite(e.to_string()))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| CertError::BadInvite("fingerprint must be 32 bytes".into()))?;
        Ok(Self(arr))
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({}…)", &self.to_hex()[..12])
    }
}

/// The server's certificate + private key.
pub struct ServerIdentity {
    pub cert: CertificateDer<'static>,
    pub key: PrivateKeyDer<'static>,
}

impl ServerIdentity {
    /// Generates a new self-signed certificate (ECDSA P-256).
    pub fn generate() -> Result<Self, CertError> {
        let ck = rcgen::generate_simple_self_signed(vec!["callout".to_string()])?;
        Ok(Self {
            cert: ck.cert.der().clone(),
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der())),
        })
    }

    /// Loads `dir/server.crt` and `dir/server.key` if present, otherwise generates and saves them.
    /// This keeps the fingerprint (and the invite link) stable across server restarts.
    pub fn load_or_generate(dir: &Path) -> Result<Self, CertError> {
        let cert_path = dir.join("server.crt");
        let key_path = dir.join("server.key");
        if cert_path.exists() && key_path.exists() {
            let cert = CertificateDer::from(std::fs::read(&cert_path)?);
            let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(std::fs::read(&key_path)?));
            return Ok(Self { cert, key });
        }
        std::fs::create_dir_all(dir)?;
        let id = Self::generate()?;
        std::fs::write(&cert_path, id.cert.as_ref())?;
        std::fs::write(&key_path, id.key.secret_der())?;
        Ok(id)
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.cert)
    }
}

/// `callout://HOST:PORT#FINGERPRINT` invite link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub addr: SocketAddr,
    pub fingerprint: Fingerprint,
}

impl Invite {
    pub fn new(addr: SocketAddr, fingerprint: Fingerprint) -> Self {
        Self { addr, fingerprint }
    }

    pub fn parse(link: &str) -> Result<Self, CertError> {
        let rest = link
            .trim()
            .strip_prefix("callout://")
            .ok_or_else(|| CertError::BadInvite("must start with callout://".into()))?;
        let (addr, fp) = rest
            .split_once('#')
            .ok_or_else(|| CertError::BadInvite("missing '#' and fingerprint".into()))?;
        let addr: SocketAddr = addr
            .parse()
            .map_err(|_| CertError::BadInvite(format!("address: {addr}")))?;
        Ok(Self {
            addr,
            fingerprint: Fingerprint::from_hex(fp)?,
        })
    }
}

impl fmt::Display for Invite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "callout://{}#{}", self.addr, self.fingerprint.to_hex())
    }
}

/// rustls verifier that accepts a certificate only with the expected fingerprint.
/// No chain, CA, domain name or date checks are done; trust comes from the fingerprint.
#[derive(Debug)]
pub struct PinnedVerifier {
    expected: Fingerprint,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl PinnedVerifier {
    pub fn new(expected: Fingerprint) -> Arc<Self> {
        Arc::new(Self {
            expected,
            provider: Arc::new(rustls::crypto::ring::default_provider()),
        })
    }
}

impl rustls::client::danger::ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        if Fingerprint::of(end_entity) == self.expected {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_roundtrip() {
        let id = ServerIdentity::generate().unwrap();
        let inv = Invite::new("127.0.0.1:4433".parse().unwrap(), id.fingerprint());
        let s = inv.to_string();
        assert!(s.starts_with("callout://127.0.0.1:4433#"));
        assert_eq!(Invite::parse(&s).unwrap(), inv);
    }

    #[test]
    fn invite_rejects_garbage() {
        assert!(Invite::parse("https://x").is_err());
        assert!(Invite::parse("callout://1.2.3.4:1").is_err());
        assert!(Invite::parse("callout://1.2.3.4:1#zz").is_err());
    }

    #[test]
    fn identity_persists_fingerprint() {
        let dir = std::env::temp_dir().join(format!("callout-cert-{}", std::process::id()));
        let a = ServerIdentity::load_or_generate(&dir).unwrap();
        let b = ServerIdentity::load_or_generate(&dir).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
