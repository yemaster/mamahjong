use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mahjong_riichi::RiichiRuleSnapshot;
use mamahjong_application::{MatchRecord, rule_display_name};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Default)]
pub struct MatchArchive {
    inner: Option<Arc<DirectoryArchive>>,
}

struct DirectoryArchive {
    directory: PathBuf,
    persisted: Mutex<HashMap<String, RecordRevision>>,
}

impl DirectoryArchive {
    /// 对局编号拼出来的归档路径。
    ///
    /// 只认字母、数字、连字符和下划线：编号是从 URL 里来的，放任 `../` 混进来就是
    /// 拿归档目录当任意文件读取的入口。
    fn record_path(&self, match_id: &str) -> Option<PathBuf> {
        if match_id.is_empty()
            || !match_id
                .chars()
                .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
        {
            return None;
        }
        Some(self.directory.join(format!("{match_id}.json")))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecordRevision {
    finished_version: Option<u64>,
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
            finished_version: record.is_finished().then(|| record.version()),
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

    /// 从归档里读一份牌谱。
    ///
    /// 内存里的对局在服务端重启之后就没了，牌谱页要翻历史局只能走这条路。请求者
    /// 必须真的在这局里坐过一个位置——绕开内存那条路不等于可以少一道检查。
    pub fn record(&self, match_id: &str, user_id: &str) -> Result<Option<Value>, ArchiveError> {
        let Some(inner) = &self.inner else {
            return Ok(None);
        };
        let Some(path) = inner.record_path(match_id) else {
            return Ok(None);
        };
        let Ok(bytes) = fs::read(path) else {
            return Ok(None);
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(ArchiveError::Decode)?;
        Ok(seat_of(&value, user_id).map(|_| value))
    }

    pub fn admin_record(&self, match_id: &str) -> Result<Option<Value>, ArchiveError> {
        let Some(inner) = &self.inner else {
            return Ok(None);
        };
        let Some(path) = inner.record_path(match_id) else {
            return Ok(None);
        };
        let Ok(bytes) = fs::read(path) else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(ArchiveError::Decode)
    }

    pub fn all_records(&self) -> Result<Vec<MatchRecordSummary>, ArchiveError> {
        let Some(inner) = &self.inner else {
            return Ok(Vec::new());
        };
        let mut records = Vec::new();
        for entry in fs::read_dir(&inner.directory).map_err(ArchiveError::Io)? {
            let entry = entry.map_err(ArchiveError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let value: Value = serde_json::from_slice(&fs::read(&path).map_err(ArchiveError::Io)?)
                .map_err(ArchiveError::Decode)?;
            if let Some(summary) = summarize(&value, file_modified_ms(&path)) {
                records.push(summary);
            }
        }
        records.sort_unstable_by(|left, right| {
            right
                .finished_at_ms
                .cmp(&left.finished_at_ms)
                .then_with(|| right.match_id.cmp(&left.match_id))
        });
        Ok(records)
    }

    /// 某个用户打过的对局记录，按结束时间倒序。
    ///
    /// 只收真的打完了的：还在进行中的对局没有名次也没有素点，列不出一行东西来。
    ///
    /// 时间取归档文件的修改时间。应用层只有一个从进程启动算起的单调时钟，
    /// 那个数拿来显示是 1970 年；归档写完的时刻就是这局打完的时刻，够列表用了。
    pub fn player_records(&self, user_id: &str) -> Result<Vec<MatchRecordSummary>, ArchiveError> {
        let Some(inner) = &self.inner else {
            return Ok(Vec::new());
        };
        let mut records = Vec::new();
        for entry in fs::read_dir(&inner.directory).map_err(ArchiveError::Io)? {
            let entry = entry.map_err(ArchiveError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let value: Value = serde_json::from_slice(&fs::read(&path).map_err(ArchiveError::Io)?)
                .map_err(ArchiveError::Decode)?;
            if seat_of(&value, user_id).is_none() {
                continue;
            }
            let Some(summary) = summarize(&value, file_modified_ms(&path)) else {
                continue;
            };
            records.push(summary);
        }
        records.sort_unstable_by(|left, right| {
            right
                .finished_at_ms
                .cmp(&left.finished_at_ms)
                .then_with(|| right.match_id.cmp(&left.match_id))
        });
        Ok(records)
    }

    pub fn player_statistics(&self, user_id: &str) -> Result<PlayerStatistics, ArchiveError> {
        let Some(inner) = &self.inner else {
            return Ok(PlayerStatistics::default());
        };
        let mut statistics = PlayerStatistics::default();
        let mut rank_sum = 0_u64;
        for entry in fs::read_dir(&inner.directory).map_err(ArchiveError::Io)? {
            let entry = entry.map_err(ArchiveError::Io)?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let value: Value =
                serde_json::from_slice(&fs::read(entry.path()).map_err(ArchiveError::Io)?)
                    .map_err(ArchiveError::Decode)?;
            let Some(players) = value.get("players").and_then(Value::as_array) else {
                continue;
            };
            let Some(player) = players
                .iter()
                .find(|player| player.get("user_id").and_then(Value::as_str) == Some(user_id))
            else {
                continue;
            };
            let Some(result) = value.get("result").filter(|result| !result.is_null()) else {
                continue;
            };
            let Some(seat) = player.get("seat").and_then(Value::as_u64) else {
                continue;
            };
            let Some(placement) =
                result
                    .get("placements")
                    .and_then(Value::as_array)
                    .and_then(|placements| {
                        placements.iter().find(|placement| {
                            placement.get("seat").and_then(Value::as_u64) == Some(seat)
                        })
                    })
            else {
                continue;
            };
            let rank = placement.get("rank").and_then(Value::as_u64).unwrap_or(0);
            let final_points = placement
                .get("points")
                .and_then(Value::as_i64)
                .and_then(|points| i32::try_from(points).ok())
                .unwrap_or(0);
            statistics.matches_played += 1;
            rank_sum += rank;
            match rank {
                1 => statistics.first_places += 1,
                2 => statistics.second_places += 1,
                3 => statistics.third_places += 1,
                4 => statistics.fourth_places += 1,
                _ => {}
            }

            let mut match_wins = 0_u32;
            let hands = value
                .get("hands")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for hand in hands {
                statistics.hands_played += 1;
                let is_winner =
                    hand.get("winners")
                        .and_then(Value::as_array)
                        .is_some_and(|winners| {
                            winners.iter().any(|winner| winner.as_u64() == Some(seat))
                        })
                        || hand
                            .get("nagashi_winners")
                            .and_then(Value::as_array)
                            .is_some_and(|winners| {
                                winners.iter().any(|winner| winner.as_u64() == Some(seat))
                            });
                if is_winner {
                    statistics.wins += 1;
                    match_wins += 1;
                    match hand.get("reason").and_then(Value::as_str) {
                        Some("tsumo") => statistics.tsumo_wins += 1,
                        Some("ron") => statistics.ron_wins += 1,
                        _ => {}
                    }
                    let gain = hand
                        .get("point_deltas")
                        .and_then(Value::as_array)
                        .and_then(|deltas| deltas.get(usize::try_from(seat).ok()?))
                        .and_then(Value::as_i64)
                        .and_then(|gain| i32::try_from(gain).ok())
                        .unwrap_or(0);
                    statistics.highest_hand_gain = statistics.highest_hand_gain.max(gain);
                }
                if hand.get("from").and_then(Value::as_u64) == Some(seat)
                    && hand
                        .get("winners")
                        .and_then(Value::as_array)
                        .is_some_and(|winners| !winners.is_empty())
                {
                    statistics.deal_ins += 1;
                }
                statistics.riichi_count += hand
                    .get("events")
                    .and_then(Value::as_array)
                    .map(|events| {
                        events
                            .iter()
                            .filter(|event| {
                                event.get("name").and_then(Value::as_str)
                                    == Some("riichi.riichi_established")
                                    && event
                                        .get("payload")
                                        .and_then(|payload| payload.get("seat"))
                                        .and_then(Value::as_u64)
                                        == Some(seat)
                            })
                            .count() as u32
                    })
                    .unwrap_or(0);
            }
            statistics.recent_matches.push(RecentMatch {
                match_id: value
                    .get("match_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                rank: u8::try_from(rank).unwrap_or_default(),
                final_points,
                hands: u32::try_from(hands.len()).unwrap_or(u32::MAX),
                wins: match_wins,
            });
        }
        if statistics.matches_played > 0 {
            statistics.average_rank = rank_sum as f64 / f64::from(statistics.matches_played);
        }
        statistics
            .recent_matches
            .sort_unstable_by(|left, right| right.match_id.cmp(&left.match_id));
        statistics.recent_matches.truncate(10);
        Ok(statistics)
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PlayerStatistics {
    matches_played: u32,
    first_places: u32,
    second_places: u32,
    third_places: u32,
    fourth_places: u32,
    hands_played: u32,
    wins: u32,
    ron_wins: u32,
    tsumo_wins: u32,
    deal_ins: u32,
    riichi_count: u32,
    highest_hand_gain: i32,
    average_rank: f64,
    recent_matches: Vec<RecentMatch>,
}

#[derive(Clone, Debug, Serialize)]
struct RecentMatch {
    match_id: String,
    rank: u8,
    final_points: i32,
    hands: u32,
    wins: u32,
}

/// 牌谱列表里的一行。
#[derive(Clone, Debug, Serialize)]
pub struct MatchRecordSummary {
    match_id: String,
    /// 归档文件的修改时间，也就是这局打完的时刻。
    finished_at_ms: u64,
    /// 旧牌谱认不出好友还是匹配，这里就是 `None`，标题只写规则部分。
    friend_match: Option<bool>,
    /// 麻将种类：规则集 ID 斜杠前那一截（`riichi`）。标题第二段写的就是它。
    rule_family: Option<String>,
    variant: Option<String>,
    match_length: Option<String>,
    /// 规则名（「ML规则」「自定义规则」……）。快照解不出来就是 `None`，标题少这一段。
    rule_name: Option<&'static str>,
    hand_count: u32,
    seats: Vec<RecordSeatSummary>,
}

/// 牌谱标题上的规则名。
///
/// 归档里躺着的是快照的 JSON，得先还原成 `RiichiRuleSnapshot` 才比得了配置——判断
/// 「改没改过预设」靠的就是这一次比较。解不出来（schema 号对不上之类）就不写：
/// 标题少一段，总好过写错一段。
pub fn record_rule_name(record: &Value) -> Option<&'static str> {
    let snapshot: RiichiRuleSnapshot =
        serde_json::from_value(record.get("rule_snapshot")?.clone()).ok()?;
    Some(rule_display_name(&snapshot))
}

#[derive(Clone, Debug, Serialize)]
struct RecordSeatSummary {
    seat: u8,
    nickname: String,
    rank: u8,
    /// 素点：终局那一刻手上的点数。
    points: i32,
    /// 算上返点和马点之后的最终得分，单位是十分之一。
    ///
    /// 牌谱列表上的「增减」写的就是这个数，不是素点减起始点数：一场对局的输赢
    /// 由马点定，同样的素点在不同名次上得分能差出好几马。
    score_tenths: i32,
}

/// 找出这份牌谱里属于该用户的座位。
fn seat_of(record: &Value, user_id: &str) -> Option<u64> {
    record
        .get("players")
        .and_then(Value::as_array)?
        .iter()
        .find(|player| player.get("user_id").and_then(Value::as_str) == Some(user_id))
        .and_then(|player| player.get("seat").and_then(Value::as_u64))
}

/// 把一份牌谱压成列表里的一行；没打完的一律跳过。
fn summarize(record: &Value, finished_at_ms: u64) -> Option<MatchRecordSummary> {
    let result = record.get("result").filter(|value| !value.is_null())?;
    let placements = result.get("placements").and_then(Value::as_array)?;
    let config = record
        .get("rule_snapshot")
        .and_then(|value| value.get("config"));
    let nicknames: HashMap<u64, String> = record
        .get("players")
        .and_then(Value::as_array)
        .map(|players| {
            players
                .iter()
                .filter_map(|player| {
                    Some((
                        player.get("seat").and_then(Value::as_u64)?,
                        player
                            .get("nickname")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut seats: Vec<_> = placements
        .iter()
        .filter_map(|placement| {
            let seat = placement.get("seat").and_then(Value::as_u64)?;
            let points = i32::try_from(placement.get("points").and_then(Value::as_i64)?).ok()?;
            Some(RecordSeatSummary {
                seat: u8::try_from(seat).ok()?,
                nickname: nicknames.get(&seat).cloned().unwrap_or_default(),
                rank: placement
                    .get("rank")
                    .and_then(Value::as_u64)
                    .and_then(|rank| u8::try_from(rank).ok())
                    .unwrap_or_default(),
                points,
                score_tenths: placement
                    .get("score_tenths")
                    .and_then(Value::as_i64)
                    .and_then(|score| i32::try_from(score).ok())
                    .unwrap_or_default(),
            })
        })
        .collect();
    seats.sort_unstable_by_key(|seat| seat.rank);

    Some(MatchRecordSummary {
        match_id: record
            .get("match_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        finished_at_ms,
        friend_match: record.get("friend_match").and_then(Value::as_bool),
        rule_family: record
            .get("rule_snapshot")
            .and_then(|snapshot| snapshot.get("rule_set_id"))
            .and_then(Value::as_str)
            .and_then(|id| id.split('/').next())
            .map(str::to_owned),
        variant: config
            .and_then(|config| config.get("variant"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        match_length: config
            .and_then(|config| config.get("match_rules"))
            .and_then(|rules| rules.get("length"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        rule_name: record_rule_name(record),
        hand_count: record
            .get("hands")
            .and_then(Value::as_array)
            .and_then(|hands| u32::try_from(hands.len()).ok())
            .unwrap_or_default(),
        seats,
    })
}

fn file_modified_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
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
    Decode(serde_json::Error),
    LockPoisoned,
    TaskFailed,
    Clock,
}

impl Display for ArchiveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "match archive I/O failed: {error}"),
            Self::Encode(error) => write!(formatter, "match archive encoding failed: {error}"),
            Self::Decode(error) => write!(formatter, "match archive decoding failed: {error}"),
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
            Self::Encode(error) | Self::Decode(error) => Some(error),
            Self::LockPoisoned | Self::TaskFailed | Self::Clock => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::UNIX_EPOCH;

    use serde_json::{Value, json};

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

    #[test]
    fn calculates_player_statistics_from_finished_records() {
        let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "mamahjong-statistics-test-{}-{suffix}",
            std::process::id()
        ));
        let archive = MatchArchive::open(&directory).expect("open archive");
        let record = json!({
            "match_id": "match-statistics",
            "players": [
                {"user_id": "user-one", "seat": 0, "nickname": "雀士"},
                {"user_id": "user-two", "seat": 1, "nickname": "对手"}
            ],
            "hands": [
                {
                    "reason": "tsumo",
                    "winners": [0],
                    "nagashi_winners": [],
                    "from": null,
                    "point_deltas": [4000, -4000],
                    "events": [{
                        "name": "riichi.riichi_established",
                        "payload": {"seat": 0}
                    }]
                },
                {
                    "reason": "ron",
                    "winners": [0],
                    "nagashi_winners": [],
                    "from": 1,
                    "point_deltas": [8000, -8000],
                    "events": []
                }
            ],
            "result": {
                "placements": [
                    {"seat": 0, "rank": 1, "points": 37000},
                    {"seat": 1, "rank": 2, "points": 13000}
                ]
            }
        });
        fs::write(
            directory.join("match-statistics.json"),
            serde_json::to_vec(&record).expect("encode record"),
        )
        .expect("write record");

        let statistics = archive
            .player_statistics("user-one")
            .expect("calculate statistics");

        assert_eq!(statistics.matches_played, 1);
        assert_eq!(statistics.first_places, 1);
        assert_eq!(statistics.hands_played, 2);
        assert_eq!(statistics.wins, 2);
        assert_eq!(statistics.tsumo_wins, 1);
        assert_eq!(statistics.ron_wins, 1);
        assert_eq!(statistics.riichi_count, 1);
        assert_eq!(statistics.highest_hand_gain, 8000);
        assert_eq!(statistics.recent_matches.len(), 1);
        drop(archive);
        fs::remove_dir_all(directory).expect("remove test archive");
    }

    fn temporary_archive(label: &str) -> (MatchArchive, PathBuf) {
        let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "mamahjong-{label}-test-{}-{suffix}",
            std::process::id()
        ));
        let archive = MatchArchive::open(&directory).expect("open archive");
        (archive, directory)
    }

    /// 写一份牌谱，并把文件修改时间钉在指定时刻——列表就是按它排序的。
    fn write_record(directory: &Path, match_id: &str, record: &Value, modified_at_ms: u64) {
        let path = directory.join(format!("{match_id}.json"));
        fs::write(&path, serde_json::to_vec(record).expect("encode record")).expect("write record");
        fs::File::options()
            .write(true)
            .open(&path)
            .expect("reopen record")
            .set_modified(UNIX_EPOCH + std::time::Duration::from_millis(modified_at_ms))
            .expect("stamp record");
    }

    /// 一份新格式的牌谱：带好友标记、规则快照和牌山。
    fn finished_record(match_id: &str) -> Value {
        json!({
            "match_id": match_id,
            "friend_match": true,
            "rule_snapshot": {
                "config": {
                    "variant": "yonma",
                    "match_rules": {"length": "hanchan", "initial_points": 25000}
                }
            },
            "players": [
                {"user_id": "user-one", "seat": 0, "nickname": "雀士"},
                {"user_id": "user-two", "seat": 1, "nickname": "对手"}
            ],
            "hands": [{"wall": {"tiles": [], "live_end": 0}, "events": []}],
            "result": {
                "placements": [
                    {"seat": 1, "rank": 2, "points": 13000, "score_tenths": -170},
                    {"seat": 0, "rank": 1, "points": 37000, "score_tenths": 320}
                ]
            }
        })
    }

    #[test]
    fn lists_player_records_newest_first_with_per_seat_points() {
        let (archive, directory) = temporary_archive("records");
        write_record(
            &directory,
            "match-old",
            &finished_record("match-old"),
            1_700_000_001_000,
        );
        write_record(
            &directory,
            "match-new",
            &finished_record("match-new"),
            1_700_000_009_000,
        );
        // 还没打完的对局排不出名次和素点，列表里不该出现。
        let mut unfinished = finished_record("match-live");
        unfinished["result"] = Value::Null;
        write_record(&directory, "match-live", &unfinished, 1_700_000_005_000);
        // 别人的对局也不该出现在我的列表里。
        let mut foreign = finished_record("match-foreign");
        foreign["players"] = json!([{"user_id": "user-three", "seat": 0, "nickname": "路人"}]);
        write_record(&directory, "match-foreign", &foreign, 1_700_000_008_000);

        let records = archive.player_records("user-one").expect("list records");

        let ids: Vec<_> = records
            .iter()
            .map(|record| record.match_id.as_str())
            .collect();
        assert_eq!(ids, ["match-new", "match-old"]);
        let newest = &records[0];
        assert_eq!(newest.finished_at_ms, 1_700_000_009_000);
        assert_eq!(newest.friend_match, Some(true));
        assert_eq!(newest.variant.as_deref(), Some("yonma"));
        assert_eq!(newest.match_length.as_deref(), Some("hanchan"));
        assert_eq!(newest.hand_count, 1);
        // 名次升序；增减写的是算过马点的最终得分，不是素点减起始点数。
        let seats: Vec<_> = newest
            .seats
            .iter()
            .map(|seat| {
                (
                    seat.seat,
                    seat.nickname.as_str(),
                    seat.points,
                    seat.score_tenths,
                )
            })
            .collect();
        assert_eq!(seats, [(0, "雀士", 37000, 320), (1, "对手", 13000, -170)]);
        drop(archive);
        fs::remove_dir_all(directory).expect("remove test archive");
    }

    /// 旧牌谱没有好友标记也没有牌山，列表照样要列得出来。
    #[test]
    fn degrades_gracefully_for_records_written_before_the_new_fields() {
        let (archive, directory) = temporary_archive("legacy-records");
        let mut legacy = finished_record("match-legacy");
        legacy
            .as_object_mut()
            .expect("record object")
            .remove("friend_match");
        legacy["hands"] = json!([{"events": []}]);
        write_record(&directory, "match-legacy", &legacy, 1_700_000_000_000);

        let records = archive.player_records("user-one").expect("list records");

        assert_eq!(records.len(), 1);
        // 认不出好友还是匹配，标题就只写规则部分。
        assert_eq!(records[0].friend_match, None);
        assert_eq!(records[0].finished_at_ms, 1_700_000_000_000);
        assert_eq!(records[0].seats.len(), 2);
        drop(archive);
        fs::remove_dir_all(directory).expect("remove test archive");
    }

    #[test]
    fn reads_one_record_only_for_players_who_sat_in_it() {
        let (archive, directory) = temporary_archive("record-read");
        write_record(
            &directory,
            "match-read",
            &finished_record("match-read"),
            1_700_000_000_000,
        );

        assert!(
            archive
                .record("match-read", "user-two")
                .expect("read record")
                .is_some()
        );
        assert!(
            archive
                .record("match-read", "user-three")
                .expect("read record")
                .is_none(),
            "没在这局里坐过就不该看到牌谱"
        );
        assert!(
            archive
                .record("missing", "user-one")
                .expect("read record")
                .is_none(),
            "归档里没有这一局"
        );
        // 编号是从 URL 里来的，别让它当成路径用。
        assert!(
            archive
                .record("../../etc/passwd", "user-one")
                .expect("read record")
                .is_none(),
            "编号里不许出现路径分隔符"
        );
        drop(archive);
        fs::remove_dir_all(directory).expect("remove test archive");
    }
}
