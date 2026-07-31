use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mamahjong_application::MatchRecord;

#[derive(Clone, Default)]
pub struct MatchArchive {
    inner: Option<Arc<DirectoryArchive>>,
}

struct DirectoryArchive {
    directory: PathBuf,
    persisted: Mutex<HashMap<String, RecordRevision>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecordRevision {
    hand_count: usize,
    finished: bool,
}

impl MatchArchive {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        fs::create_dir_all(directory.as_ref()).map_err(ArchiveError::Io)?;
        let directory = fs::canonicalize(directory.as_ref()).map_err(ArchiveError::Io)?;
        verify_writable(&directory)?;
        Ok(Self {
            inner: Some(Arc::new(DirectoryArchive {
                directory,
                persisted: Mutex::new(HashMap::new()),
            })),
        })
    }

    pub fn persist(&self, record: &MatchRecord) -> Result<(), ArchiveError> {
        let Some(inner) = &self.inner else {
            return Ok(());
        };
        let revision = RecordRevision {
            hand_count: record.hand_count(),
            finished: record.is_finished(),
        };
        let mut persisted = inner
            .persisted
            .lock()
            .map_err(|_| ArchiveError::LockPoisoned)?;
        if persisted.get(record.match_id()) == Some(&revision) {
            return Ok(());
        }

        let bytes = serde_json::to_vec_pretty(record).map_err(ArchiveError::Encode)?;
        let target = inner.directory.join(format!("{}.json", record.match_id()));
        let temporary = inner.directory.join(format!(".{}.tmp", record.match_id()));
        let mut file = File::create(&temporary).map_err(ArchiveError::Io)?;
        file.write_all(&bytes).map_err(ArchiveError::Io)?;
        file.write_all(b"\n").map_err(ArchiveError::Io)?;
        file.sync_all().map_err(ArchiveError::Io)?;
        fs::rename(&temporary, &target).map_err(ArchiveError::Io)?;
        OpenOptions::new()
            .read(true)
            .open(&inner.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(ArchiveError::Io)?;
        persisted.insert(record.match_id().to_owned(), revision);
        Ok(())
    }
}

fn verify_writable(directory: &Path) -> Result<(), ArchiveError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ArchiveError::Clock)?
        .as_nanos();
    let probe = directory.join(format!(
        ".mamahjong-write-test-{}-{timestamp}",
        std::process::id()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(ArchiveError::Io)?;
    file.sync_all().map_err(ArchiveError::Io)?;
    fs::remove_file(probe).map_err(ArchiveError::Io)
}

#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    Encode(serde_json::Error),
    LockPoisoned,
    TaskFailed,
    Clock,
}

impl Display for ArchiveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "match archive I/O failed: {error}"),
            Self::Encode(error) => write!(formatter, "match archive encoding failed: {error}"),
            Self::LockPoisoned => formatter.write_str("match archive lock is poisoned"),
            Self::TaskFailed => formatter.write_str("match archive task failed"),
            Self::Clock => formatter.write_str("system clock is before the Unix epoch"),
        }
    }
}

impl Error for ArchiveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::LockPoisoned | Self::TaskFailed | Self::Clock => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::MatchArchive;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn opens_archive_directory() {
        let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "mamahjong-archive-test-{}-{suffix}",
            std::process::id()
        ));

        let archive = MatchArchive::open(&directory).expect("open archive");

        assert!(archive.inner.is_some());
        assert!(directory.is_dir());
        std::fs::remove_dir_all(directory).expect("remove test archive");
    }
}
