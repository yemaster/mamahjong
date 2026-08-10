use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const SCHEMA: &str = "audit_event.v1";
const INITIAL_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MEMORY_LIMIT: usize = 2_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditDraft {
    pub severity: &'static str,
    pub category: &'static str,
    pub action: &'static str,
    pub actor_id: Option<String>,
    pub target_type: &'static str,
    pub target_id: Option<String>,
    pub outcome: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuditEvent {
    pub schema: String,
    pub sequence: u64,
    pub occurred_at: String,
    pub severity: String,
    pub category: String,
    pub action: String,
    pub actor_id: Option<String>,
    pub target_type: String,
    pub target_id: Option<String>,
    pub outcome: String,
    pub detail: String,
    pub previous_hash: String,
    pub entry_hash: String,
}

#[derive(Clone)]
pub struct AuditLog {
    inner: Arc<Mutex<AuditState>>,
}

struct AuditState {
    file: Option<File>,
    path: Option<PathBuf>,
    entries: VecDeque<AuditEvent>,
    next_sequence: u64,
    last_hash: String,
}

impl AuditLog {
    #[must_use]
    pub fn memory() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AuditState {
                file: None,
                path: None,
                entries: VecDeque::new(),
                next_sequence: 1,
                last_hash: INITIAL_HASH.to_owned(),
            })),
        }
    }

    pub fn open(data_directory: impl AsRef<Path>) -> Result<Self, AuditError> {
        let directory = data_directory.as_ref().join("audit");
        fs::create_dir_all(&directory).map_err(AuditError::Io)?;
        let path = directory.join("audit.jsonl");
        let mut entries = VecDeque::new();
        let mut next_sequence = 1_u64;
        let mut last_hash = INITIAL_HASH.to_owned();

        if path.exists() {
            let reader = BufReader::new(File::open(&path).map_err(AuditError::Io)?);
            for (index, line) in reader.lines().enumerate() {
                let line_number = index + 1;
                let line = line.map_err(AuditError::Io)?;
                if line.trim().is_empty() {
                    return Err(AuditError::Integrity { line: line_number });
                }
                let event: AuditEvent = serde_json::from_str(&line)
                    .map_err(|_| AuditError::Integrity { line: line_number })?;
                if event.schema != SCHEMA
                    || event.sequence != next_sequence
                    || event.previous_hash != last_hash
                    || event.entry_hash != calculate_hash(&event)?
                {
                    return Err(AuditError::Integrity { line: line_number });
                }
                next_sequence = next_sequence
                    .checked_add(1)
                    .ok_or(AuditError::SequenceOverflow)?;
                last_hash.clone_from(&event.entry_hash);
                push_recent(&mut entries, event);
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(AuditError::Io)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(AuditState {
                file: Some(file),
                path: Some(path),
                entries,
                next_sequence,
                last_hash,
            })),
        })
    }

    pub fn record(&self, draft: AuditDraft) -> Result<AuditEvent, AuditError> {
        let mut state = self.inner.lock().map_err(|_| AuditError::LockPoisoned)?;
        let occurred_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(AuditError::Time)?;
        let mut event = AuditEvent {
            schema: SCHEMA.to_owned(),
            sequence: state.next_sequence,
            occurred_at,
            severity: sanitize(draft.severity, 16),
            category: sanitize(draft.category, 32),
            action: sanitize(draft.action, 96),
            actor_id: draft.actor_id.map(|value| sanitize(&value, 128)),
            target_type: sanitize(draft.target_type, 32),
            target_id: draft.target_id.map(|value| sanitize(&value, 128)),
            outcome: sanitize(draft.outcome, 24),
            detail: sanitize(&draft.detail, 240),
            previous_hash: state.last_hash.clone(),
            entry_hash: String::new(),
        };
        event.entry_hash = calculate_hash(&event)?;
        let mut encoded = serde_json::to_vec(&event).map_err(AuditError::Encode)?;
        encoded.push(b'\n');
        if let Some(file) = &mut state.file {
            file.write_all(&encoded).map_err(AuditError::Io)?;
            file.sync_data().map_err(AuditError::Io)?;
        }
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or(AuditError::SequenceOverflow)?;
        state.last_hash.clone_from(&event.entry_hash);
        push_recent(&mut state.entries, event.clone());
        Ok(event)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<AuditEvent>, AuditError> {
        let state = self.inner.lock().map_err(|_| AuditError::LockPoisoned)?;
        Ok(state
            .entries
            .iter()
            .rev()
            .take(limit.min(MEMORY_LIMIT))
            .cloned()
            .collect())
    }

    pub fn count(&self) -> Result<u64, AuditError> {
        let state = self.inner.lock().map_err(|_| AuditError::LockPoisoned)?;
        Ok(state.next_sequence.saturating_sub(1))
    }

    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.inner.lock().ok().and_then(|state| state.path.clone())
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::memory()
    }
}

fn calculate_hash(event: &AuditEvent) -> Result<String, AuditError> {
    #[derive(Serialize)]
    struct HashMaterial<'a> {
        schema: &'a str,
        sequence: u64,
        occurred_at: &'a str,
        severity: &'a str,
        category: &'a str,
        action: &'a str,
        actor_id: &'a Option<String>,
        target_type: &'a str,
        target_id: &'a Option<String>,
        outcome: &'a str,
        detail: &'a str,
        previous_hash: &'a str,
    }

    let material = HashMaterial {
        schema: &event.schema,
        sequence: event.sequence,
        occurred_at: &event.occurred_at,
        severity: &event.severity,
        category: &event.category,
        action: &event.action,
        actor_id: &event.actor_id,
        target_type: &event.target_type,
        target_id: &event.target_id,
        outcome: &event.outcome,
        detail: &event.detail,
        previous_hash: &event.previous_hash,
    };
    let bytes = serde_json::to_vec(&material).map_err(AuditError::Encode)?;
    let digest = Sha256::digest(bytes);
    Ok(hex(&digest))
}

fn push_recent(entries: &mut VecDeque<AuditEvent>, event: AuditEvent) {
    if entries.len() == MEMORY_LIMIT {
        entries.pop_front();
    }
    entries.push_back(event);
}

fn sanitize(value: &str, max_characters: usize) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(max_characters)
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[derive(Debug)]
pub enum AuditError {
    Io(std::io::Error),
    Encode(serde_json::Error),
    Time(time::error::Format),
    Integrity { line: usize },
    LockPoisoned,
    SequenceOverflow,
    TaskFailed,
}

impl Display for AuditError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "audit log I/O failed: {error}"),
            Self::Encode(error) => write!(formatter, "audit log encoding failed: {error}"),
            Self::Time(error) => write!(formatter, "audit timestamp failed: {error}"),
            Self::Integrity { line } => {
                write!(formatter, "audit log integrity check failed at line {line}")
            }
            Self::LockPoisoned => formatter.write_str("audit log lock is poisoned"),
            Self::SequenceOverflow => formatter.write_str("audit sequence overflow"),
            Self::TaskFailed => formatter.write_str("audit task failed"),
        }
    }
}

impl Error for AuditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::Time(error) => Some(error),
            Self::Integrity { .. }
            | Self::LockPoisoned
            | Self::SequenceOverflow
            | Self::TaskFailed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{AuditDraft, AuditError, AuditLog};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn draft(detail: &str) -> AuditDraft {
        AuditDraft {
            severity: "info",
            category: "admin",
            action: "admin.test",
            actor_id: Some("user_1".to_owned()),
            target_type: "room",
            target_id: Some("room_1".to_owned()),
            outcome: "success",
            detail: detail.to_owned(),
        }
    }

    #[test]
    fn persistent_log_reopens_and_preserves_hash_chain() {
        let directory = test_directory("reopen");
        let log = AuditLog::open(&directory).expect("audit log");
        let first = log.record(draft("第一条")).expect("first");
        let second = log.record(draft("第二条")).expect("second");
        drop(log);

        let reopened = AuditLog::open(&directory).expect("reopen");
        let entries = reopened.recent(10).expect("entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], second);
        assert_eq!(entries[1], first);
        std::fs::remove_dir_all(directory).expect("remove directory");
    }

    #[test]
    fn tampered_log_is_rejected() {
        let directory = test_directory("tamper");
        let log = AuditLog::open(&directory).expect("audit log");
        log.record(draft("原始内容")).expect("record");
        let path = log.path().expect("path");
        drop(log);
        let content = std::fs::read_to_string(&path).expect("read");
        std::fs::write(&path, content.replace("原始内容", "篡改内容")).expect("tamper");

        assert!(matches!(
            AuditLog::open(&directory),
            Err(AuditError::Integrity { line: 1 })
        ));
        std::fs::remove_dir_all(directory).expect("remove directory");
    }

    fn test_directory(label: &str) -> std::path::PathBuf {
        let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mamahjong-audit-{label}-{}-{suffix}",
            std::process::id()
        ))
    }
}
