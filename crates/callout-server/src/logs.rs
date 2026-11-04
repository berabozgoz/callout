//! Diagnostic logs uploaded by users who agreed to it (see `callout_proto::LogUpload`).
//!
//! Files go to `<data>/logs/<UTC time>_<nickname>_<device>.log`, `device` being the first
//! 8 hex digits of the device key (nicknames are not unique). Limits:
//! - one upload per device every `MIN_INTERVAL`;
//! - at most `MAX_LOG_UPLOAD` per file (enforced while reading the stream);
//! - files older than `RETENTION` are deleted, and the oldest ones go first when the folder
//!   passes `MAX_TOTAL` (checked hourly and before each store).
//!
//! `CALLOUT_LOG_UPLOADS=0` turns uploads off (the client is told so). The app tells the user
//! the retention below, so keep the two in step.

use callout_proto::{LogUploadReply, PublicKey};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const RETENTION: Duration = Duration::from_secs(30 * 24 * 3600);
pub const MAX_TOTAL: u64 = 200 * 1024 * 1024;
pub const MIN_INTERVAL: Duration = Duration::from_secs(10 * 60);

pub struct LogStore {
    dir: PathBuf,
    enabled: bool,
    last_upload: Mutex<HashMap<PublicKey, Instant>>,
}

impl LogStore {
    pub fn new(dir: PathBuf, enabled: bool) -> Self {
        Self {
            dir,
            enabled,
            last_upload: Mutex::new(HashMap::new()),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Stores one upload. `log` has already been checked against `MAX_LOG_UPLOAD`.
    pub fn store(
        &self,
        pubkey: &PublicKey,
        nickname: &str,
        app_version: &str,
        log: &[u8],
    ) -> LogUploadReply {
        if !self.enabled {
            return LogUploadReply::Disabled;
        }
        {
            let mut last = self.last_upload.lock().unwrap();
            let now = Instant::now();
            if last
                .get(pubkey)
                .is_some_and(|t| now.duration_since(*t) < MIN_INTERVAL)
            {
                return LogUploadReply::RateLimited;
            }
            last.insert(*pubkey, now);
        }
        self.cleanup_with_room_for(log.len() as u64);
        let device: String = pubkey[..4].iter().map(|b| format!("{b:02x}")).collect();
        let now = utc_now();
        let path = self.dir.join(format!(
            "{}_{}_{device}.log",
            now.replace(':', "-"),
            file_safe(nickname)
        ));
        let res = (|| {
            std::fs::create_dir_all(&self.dir)?;
            let mut f = std::fs::File::create(&path)?;
            writeln!(f, "# CallOut client log")?;
            writeln!(f, "# nickname: {nickname}")?;
            writeln!(f, "# device: {device}")?;
            writeln!(f, "# app version: {app_version}")?;
            writeln!(f, "# received: {now}")?;
            writeln!(f)?;
            f.write_all(log)
        })();
        match res {
            Ok(()) => {
                tracing::info!(nickname, device, bytes = log.len(), path = %path.display(), "log stored");
                LogUploadReply::Stored
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), "could not store log: {e}");
                let _ = std::fs::remove_file(&path);
                LogUploadReply::Failed
            }
        }
    }

    /// Deletes logs past the retention time, then the oldest ones until the folder is under
    /// `MAX_TOTAL`.
    pub fn cleanup(&self) {
        self.cleanup_with_room_for(0);
    }

    fn cleanup_with_room_for(&self, incoming: u64) {
        let mut files = list_logs(&self.dir);
        let now = SystemTime::now();
        files.retain(|(path, modified, _)| {
            let expired = now.duration_since(*modified).unwrap_or_default() > RETENTION;
            if expired {
                let _ = std::fs::remove_file(path);
            }
            !expired
        });
        files.sort_by_key(|(_, modified, _)| *modified);
        let mut total: u64 = files.iter().map(|(_, _, len)| len).sum::<u64>() + incoming;
        for (path, _, len) in files {
            if total <= MAX_TOTAL {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    }
}

fn list_logs(dir: &Path) -> Vec<(PathBuf, SystemTime, u64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((e.path(), m.modified().ok()?, m.len()))
        })
        .collect()
}

/// Letters, digits, `-` and `_` only, at most 24 characters.
fn file_safe(nickname: &str) -> String {
    let s: String = nickname
        .chars()
        .filter(|c| !c.is_control())
        .take(24)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "user".into()
    } else {
        s
    }
}

/// `2026-11-04T21:30:05Z`
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    utc(secs)
}

fn utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 → civil date (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("callout-logs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn utc_formatting() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc(1_793_771_917), "2026-11-04T05:58:37Z");
    }

    #[test]
    fn nicknames_become_safe_file_names() {
        assert_eq!(file_safe("cocart"), "cocart");
        assert_eq!(file_safe("../../etc/passwd"), "______etc_passwd");
        assert_eq!(file_safe("a b\\c:d"), "a_b_c_d");
        assert_eq!(file_safe("Çağrı"), "Çağrı");
        assert_eq!(file_safe(""), "user");
        assert_eq!(file_safe(&"x".repeat(40)).len(), 24);
    }

    #[test]
    fn stores_once_per_interval_per_device() {
        let dir = temp_dir("rate");
        let store = LogStore::new(dir.clone(), true);
        let log = b"line 1\nline 2\n";
        assert_eq!(
            store.store(&[1; 32], "bera", "0.1.16", log),
            LogUploadReply::Stored
        );
        assert_eq!(
            store.store(&[1; 32], "bera", "0.1.16", log),
            LogUploadReply::RateLimited
        );
        assert_eq!(
            store.store(&[2; 32], "cocart", "0.1.16", log),
            LogUploadReply::Stored
        );
        let files = list_logs(&dir);
        assert_eq!(files.len(), 2);
        let text = std::fs::read_to_string(&files[0].0).unwrap();
        assert!(text.starts_with("# CallOut client log\n"), "{text}");
        assert!(text.ends_with("line 1\nline 2\n"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_store_writes_nothing() {
        let dir = temp_dir("off");
        let store = LogStore::new(dir.clone(), false);
        assert_eq!(
            store.store(&[1; 32], "bera", "0.1.16", b"x"),
            LogUploadReply::Disabled
        );
        assert!(list_logs(&dir).is_empty());
    }

    #[test]
    fn cleanup_keeps_the_newest_under_the_cap() {
        let dir = temp_dir("cap");
        std::fs::create_dir_all(&dir).unwrap();
        // Three 10-byte files, and an upload that leaves room for only one of them.
        for (i, name) in ["a.log", "b.log", "c.log"].iter().enumerate() {
            std::fs::write(dir.join(name), vec![b'x'; 10]).unwrap();
            std::thread::sleep(Duration::from_millis(20 * (i as u64 + 1)));
        }
        let store = LogStore::new(dir.clone(), true);
        store.cleanup_with_room_for(MAX_TOTAL - 15);
        let mut left: Vec<String> = list_logs(&dir)
            .into_iter()
            .map(|(p, _, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["c.log"], "the two oldest go first");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
