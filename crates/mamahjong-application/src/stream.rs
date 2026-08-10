use mahjong_riichi::{HandEvent, Seat};
use serde_json::{Value, json};

use crate::game::GameEventRecord;
use crate::record::{draw_source_name, event_payload};

/// Upper bound of events returned by a single cursor read.
pub const MATCH_EVENT_PAGE_LIMIT: usize = 512;

/// One match event already redacted for a single observer.
#[derive(Clone, Debug)]
pub struct MatchEvent {
    sequence: u64,
    hand_index: u32,
    name: &'static str,
    event_version: u8,
    payload: Value,
}

impl MatchEvent {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn hand_index(&self) -> u32 {
        self.hand_index
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn event_version(&self) -> u8 {
        self.event_version
    }

    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    pub(crate) fn redacted(record: &GameEventRecord, observer: Seat) -> Self {
        let (name, payload) = redacted_payload(record.event(), observer);
        Self {
            sequence: record.sequence(),
            hand_index: record.hand_index(),
            name,
            event_version: 1,
            payload,
        }
    }
}

/// A cursor read of a match event stream from one observer's perspective.
#[derive(Clone, Debug)]
pub struct MatchEventPage {
    version: u64,
    latest_sequence: u64,
    events: Box<[MatchEvent]>,
}

impl MatchEventPage {
    pub(crate) const fn new(version: u64, latest_sequence: u64, events: Box<[MatchEvent]>) -> Self {
        Self {
            version,
            latest_sequence,
            events,
        }
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Newest sequence held by the match, independent of the page limit.
    #[must_use]
    pub const fn latest_sequence(&self) -> u64 {
        self.latest_sequence
    }

    #[must_use]
    pub fn events(&self) -> &[MatchEvent] {
        &self.events
    }
}

/// Rewrites the three events that carry hidden tiles; everything else is public fact.
fn redacted_payload(event: &HandEvent, observer: Seat) -> (&'static str, Value) {
    match event {
        HandEvent::InitialHandDealt { seat, tiles } if *seat != observer => (
            "riichi.initial_hand_dealt",
            json!({
                "seat": seat.index(),
                "tile_count": tiles.len(),
            }),
        ),
        HandEvent::TileDrawn {
            seat,
            source,
            remaining_live_draws,
            ..
        } if *seat != observer => (
            "riichi.tile_drawn",
            json!({
                "seat": seat.index(),
                "source": draw_source_name(*source),
                "remaining_live_draws": remaining_live_draws,
            }),
        ),
        HandEvent::FuritenChanged { seat, .. } if *seat != observer => {
            ("riichi.furiten_changed", json!({"seat": seat.index()}))
        }
        event => event_payload(event),
    }
}

#[cfg(test)]
mod tests {
    use mahjong_core::MatchId;
    use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
    use serde_json::Value;

    use crate::{
        Application, CreateRoom, ErrorCode, RegisterUser, RoomRuleSelection, RoomVisibility, User,
    };

    /// Builds a started sanma match; the returned users are seated in order.
    fn started_match(suffix: &str) -> (Application, Vec<User>, MatchId) {
        let application = Application::new();
        let players = (0..3)
            .map(|index| {
                application
                    .register(RegisterUser {
                        login_name: format!("stream_{suffix}_{index}"),
                        password: "correct horse battery staple".to_owned(),
                        nickname: format!("雀士{index}"),
                    })
                    .expect("register")
                    .0
            })
            .collect::<Vec<_>>();
        let mut room = application
            .create_room(
                players[0].id(),
                CreateRoom {
                    name: "事件流".to_owned(),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant: RiichiVariant::Sanma,
                        request: RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        for player in &players[1..] {
            room = application
                .join_room(player.id(), room.id(), room.version())
                .expect("join");
        }
        for player in &players {
            room = application
                .set_ready(player.id(), room.id(), room.version(), true)
                .expect("ready");
        }
        let (_, match_id) = application
            .start_room(players[0].id(), room.id(), room.version(), 0)
            .expect("start");
        (application, players, match_id)
    }

    fn payload_of<'a>(page: &'a super::MatchEventPage, name: &str, seat: u64) -> &'a Value {
        page.events()
            .iter()
            .find(|event| event.name() == name && event.payload()["seat"].as_u64() == Some(seat))
            .map(super::MatchEvent::payload)
            .unwrap_or_else(|| panic!("{name} for seat {seat}"))
    }

    #[test]
    fn only_match_players_can_read_the_event_stream() {
        let (application, players, match_id) = started_match("outsider");
        let outsider = application
            .register(RegisterUser {
                login_name: "stream_outsider_watcher".to_owned(),
                password: "correct horse battery staple".to_owned(),
                nickname: "旁观".to_owned(),
            })
            .expect("register")
            .0;

        assert!(
            application
                .match_events(players[0].id(), &match_id, 0)
                .is_ok()
        );
        assert_eq!(
            application
                .match_events(outsider.id(), &match_id, 0)
                .expect_err("not a player")
                .code(),
            ErrorCode::NotMatchPlayer
        );
    }

    #[test]
    fn the_cursor_returns_only_newer_events() {
        let (application, players, match_id) = started_match("cursor");
        let full = application
            .match_events(players[0].id(), &match_id, 0)
            .expect("full page");

        assert!(!full.events().is_empty());
        assert_eq!(full.events()[0].sequence(), 1);
        assert_eq!(
            full.events().last().expect("last event").sequence(),
            full.latest_sequence()
        );
        assert!(
            full.events()
                .windows(2)
                .all(|pair| pair[0].sequence() < pair[1].sequence())
        );

        let tail = application
            .match_events(players[0].id(), &match_id, full.latest_sequence())
            .expect("empty page");
        assert!(tail.events().is_empty());
        assert_eq!(tail.latest_sequence(), full.latest_sequence());

        let partial = application
            .match_events(players[0].id(), &match_id, 1)
            .expect("partial page");
        assert_eq!(partial.events()[0].sequence(), 2);
        assert_eq!(partial.events().len(), full.events().len() - 1);
    }

    #[test]
    fn hidden_tiles_are_redacted_for_everyone_but_their_owner() {
        let (application, players, match_id) = started_match("redaction");
        let view = application
            .match_view(players[0].id(), &match_id)
            .expect("match view");
        let dealer_id = view.players()[0].player().user_id();
        let owner = players
            .iter()
            .find(|player| player.id() == dealer_id)
            .expect("dealer player");
        let observer = players
            .iter()
            .find(|player| player.id() != dealer_id)
            .expect("other player");
        let own = application
            .match_events(owner.id(), &match_id, 0)
            .expect("own page");
        let other = application
            .match_events(observer.id(), &match_id, 0)
            .expect("other page");

        let dealt = payload_of(&own, "riichi.initial_hand_dealt", 0);
        assert_eq!(dealt["tiles"].as_array().expect("own tiles").len(), 13);
        assert!(dealt.get("tile_count").is_none());

        let hidden = payload_of(&other, "riichi.initial_hand_dealt", 0);
        assert!(hidden.get("tiles").is_none());
        assert_eq!(hidden["tile_count"], 13);

        let drawn = payload_of(&own, "riichi.tile_drawn", 0);
        assert!(drawn["tile"].is_object());
        let unseen = payload_of(&other, "riichi.tile_drawn", 0);
        assert!(unseen.get("tile").is_none());
        assert_eq!(unseen["source"], drawn["source"]);
        assert_eq!(
            unseen["remaining_live_draws"],
            drawn["remaining_live_draws"]
        );
    }

    #[test]
    fn every_observer_shares_sequences_and_public_events() {
        let (application, players, match_id) = started_match("shared");
        let pages = players
            .iter()
            .map(|player| {
                application
                    .match_events(player.id(), &match_id, 0)
                    .expect("page")
            })
            .collect::<Vec<_>>();

        let sequences = pages
            .iter()
            .map(|page| {
                page.events()
                    .iter()
                    .map(|event| (event.sequence(), event.name()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert!(sequences.windows(2).all(|pair| pair[0] == pair[1]));

        let started = pages
            .iter()
            .map(|page| {
                page.events()
                    .iter()
                    .find(|event| event.name() == "riichi.hand_started")
                    .expect("hand started")
                    .payload()
                    .clone()
            })
            .collect::<Vec<_>>();
        assert!(started.windows(2).all(|pair| pair[0] == pair[1]));
    }
}
