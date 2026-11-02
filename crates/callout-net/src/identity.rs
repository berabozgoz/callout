//! Client identity: Ed25519 device key. There is no password; key + nickname = user.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("key file: {0}")]
    Io(#[from] std::io::Error),
    #[error("key file is corrupt")]
    Corrupt,
    #[error("invalid signature")]
    BadSignature,
}

pub struct DeviceKey {
    signing: SigningKey,
}

impl DeviceKey {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut rand::rngs::OsRng),
        }
    }

    /// Loads `path` if present, otherwise generates and saves it (32-byte raw secret key).
    pub fn load_or_generate(path: &Path) -> Result<Self, IdentityError> {
        if path.exists() {
            let bytes = std::fs::read(path)?;
            let arr: [u8; 32] = bytes.try_into().map_err(|_| IdentityError::Corrupt)?;
            return Ok(Self {
                signing: SigningKey::from_bytes(&arr),
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let key = Self::generate();
        std::fs::write(path, key.signing.to_bytes())?;
        Ok(key)
    }

    pub fn public_key(&self) -> callout_proto::PublicKey {
        self.signing.verifying_key().to_bytes()
    }

    pub fn sign(&self, msg: &[u8]) -> callout_proto::Signature {
        self.signing.sign(msg).to_bytes()
    }
}

/// Server side: verifies the signature over `challenge` with `pubkey`.
pub fn verify(
    pubkey: &callout_proto::PublicKey,
    challenge: &[u8],
    signature: &callout_proto::Signature,
) -> Result<(), IdentityError> {
    let vk = VerifyingKey::from_bytes(pubkey).map_err(|_| IdentityError::BadSignature)?;
    let sig = Signature::from_bytes(signature);
    vk.verify(challenge, &sig)
        .map_err(|_| IdentityError::BadSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let k = DeviceKey::generate();
        let msg = [7u8; 32];
        let sig = k.sign(&msg);
        assert!(verify(&k.public_key(), &msg, &sig).is_ok());
        assert!(verify(&k.public_key(), &[0u8; 32], &sig).is_err());
    }
}
