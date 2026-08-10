use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct AuthResponse {
    pub user: UserView,
    pub session: SessionView,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct UserView {
    pub id: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub profile: Option<ProfileView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProfileView {
    #[serde(default)]
    pub nickname: String,
}

impl UserView {
    pub fn nickname(&self) -> &str {
        if !self.nickname.is_empty() {
            &self.nickname
        } else if let Some(profile) = &self.profile {
            &profile.nickname
        } else {
            "未知"
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionView {
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct RoomView {
    pub id: String,
    pub version: u64,
    pub owner_user_id: String,
    pub name: String,
    pub lifecycle: String,
    pub members: Vec<RoomMemberView>,
    pub active_match_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct RoomMemberView {
    pub user_id: String,
    pub seat: u8,
    pub nickname: String,
    pub ready: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct StartMatchResponse {
    pub match_id: String,
}

/// The match-view wire format is shared by riichi and impact — `variant_kind`
/// tells the client which optional fields to read.
#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct MatchView {
    /// `"riichi"` or `"impact"`.
    #[serde(default)]
    pub variant_kind: String,
    pub id: String,
    #[serde(default)]
    pub room_id: String,
    pub version: u64,
    pub hand_index: u32,
    pub observer_seat: u8,
    pub progress: ProgressView,
    pub phase: MatchPhase,
    #[serde(default)]
    pub dora_indicators: Vec<TileView>,
    /// Impact: the single joker indicator tile (upper-left).
    #[serde(default)]
    pub joker_indicator: Option<TileView>,
    /// Impact: the tile code that marks a joker (the indicator's successor).
    #[serde(default)]
    pub joker_code: Option<String>,
    /// Impact: consecutive dealer wins. Riichi uses 0.
    #[serde(default)]
    pub dealer_streak: Option<u32>,
    /// Impact: kan-point movement that may need an animation ack.
    #[serde(default)]
    pub last_kan: Option<KanPointsView>,
    pub players: Vec<MatchPlayerView>,
    #[serde(default)]
    pub available_reactions: Vec<ReactionOptionView>,
    #[serde(default)]
    pub turn_actions: TurnActionsView,
    /// Seats that finished loading the match assets. The server refuses every
    /// command until all of them are in here.
    #[serde(default)]
    pub assets_ready_seats: Vec<u8>,
    /// Whether the match was scrapped because a seat never finished loading.
    #[serde(default)]
    pub terminated_by_asset_timeout: bool,
    /// Seats that acknowledged the opening deal of the current hand. The hand's
    /// clocks stay disarmed until every seat is in here.
    #[serde(default)]
    pub opening_ready_seats: Vec<u8>,
    /// Present while a finished hand is waiting to be acknowledged.
    #[serde(default)]
    pub hand_settlement: Option<HandSettlementView>,
    pub result: Option<MatchResultView>,
    #[serde(default)]
    pub exit_vote: Option<ExitVoteView>,
    #[serde(default)]
    pub terminated_by_exit_vote: bool,
}

impl MatchView {
    #[allow(dead_code)]
    pub const VARIANT_RIICHI: &'static str = "riichi";
    #[allow(dead_code)]
    pub const VARIANT_IMPACT: &'static str = "impact";

    pub fn observer(&self) -> Result<&MatchPlayerView, String> {
        self.players
            .iter()
            .find(|player| player.seat == self.observer_seat)
            .ok_or_else(|| "观察者座位不存在".to_owned())
    }

    pub fn is_impact(&self) -> bool {
        self.variant_kind == Self::VARIANT_IMPACT
    }

    /// The tile code that means "joker" in this match, or `None` for riichi.
    pub fn joker_code(&self) -> Option<&str> {
        self.joker_code.as_deref()
    }

    /// Whether this seat still owes the server an asset-load report.
    pub fn needs_assets_ready(&self) -> bool {
        !self.assets_ready_seats.contains(&self.observer_seat)
    }

    /// Whether somebody at the table is still loading the match assets.
    pub fn assets_loading(&self) -> bool {
        self.assets_ready_seats.len() < self.players.len()
    }

    /// Whether this seat still owes the server an opening acknowledgement.
    pub fn needs_opening_ready(&self) -> bool {
        !self.opening_ready_seats.contains(&self.observer_seat)
    }

    /// Whether somebody at the table is still playing the opening deal.
    pub fn opening_in_progress(&self) -> bool {
        self.opening_ready_seats.len() < self.players.len()
    }

    /// The pending kan animation this seat has not yet acknowledged.
    ///
    /// Impact mahjong only — every kan (and first-round-repeat) pauses the table
    /// until every seat sends `impact.kan_animation_played`.  The server
    /// auto-advances past it on a timeout, but the bot should ack promptly.
    pub fn unplayed_kan(&self) -> Option<&KanPointsView> {
        self.last_kan.as_ref()
    }

    /// The pending settlement this seat has not yet reported as played out.
    pub fn unplayed_settlement(&self) -> Option<&HandSettlementView> {
        self.hand_settlement
            .as_ref()
            .filter(|settlement| !settlement.played_seats.contains(&self.observer_seat))
    }

    /// The pending settlement when this seat has not acknowledged it yet.
    ///
    /// Only meaningful once the server opened the confirm window — it rejects
    /// confirmations sent while somebody is still playing the animation.
    pub fn unconfirmed_settlement(&self) -> Option<&HandSettlementView> {
        self.hand_settlement.as_ref().filter(|settlement| {
            settlement.confirm_remaining_ms.is_some()
                && !settlement.confirmed_seats.contains(&self.observer_seat)
        })
    }

    /// Whether this seat still owes the server an exit vote.
    pub fn needs_exit_vote(&self) -> bool {
        self.exit_vote.as_ref().is_some_and(|vote| {
            vote.initiator_seat != self.observer_seat
                && vote
                    .votes
                    .get(usize::from(self.observer_seat))
                    .is_none_or(Option::is_none)
        })
    }
}

/// Legal turn actions the server computed for the observer.
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(dead_code)]
pub struct TurnActionsView {
    #[serde(default)]
    pub can_tsumo: bool,
    /// Riichi: tile ids the player may riichi-discard.
    #[serde(default)]
    pub riichi_discard_tile_ids: Vec<u16>,
    /// Riichi: one entry per legal concealed kan, each holding all four tile ids.
    #[serde(default)]
    pub concealed_kan_tile_ids: Vec<[u16; 4]>,
    #[serde(default)]
    pub can_nine_terminals: bool,
    /// Impact: "discard this → you're tenpai, waiting on these tile codes."
    #[serde(default)]
    pub tenpai_discard_hints: Vec<DiscardWaitHintView>,
    /// Impact: tile codes eligible for concealed kan.
    #[serde(default)]
    pub impact_concealed_kan_tile_codes: Option<Vec<String>>,
    /// Impact: meld ids eligible for added kan.
    #[serde(default)]
    pub impact_added_kan_meld_ids: Option<Vec<u16>>,
    /// Impact: the player may declare an indicator concealed kan (kan points only).
    #[serde(default)]
    pub impact_indicator_concealed_kan: Option<bool>,
}

/// Impact: one discard → wait list pair produced by the server engine.
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(dead_code)]
pub struct DiscardWaitHintView {
    pub tile_id: u16,
    #[serde(default)]
    pub waiting_tiles: Vec<WaitingTileView>,
}

/// Impact: a tile the discarder would be waiting on after one specific discard.
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(dead_code)]
pub struct WaitingTileView {
    pub code: String,
    #[serde(default)]
    pub has_yaku: bool,
}

/// Impact: a kan-point movement that may trigger an animation handshake.
#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct KanPointsView {
    pub id: u64,
    pub seat: u8,
    pub kind: String,
    pub deltas: Vec<i32>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct HandSettlementView {
    pub reason: String,
    #[serde(default)]
    pub tenpai_seats: Vec<u8>,
    pub point_deltas: Vec<i32>,
    pub points_before: Vec<i32>,
    pub points_after: Vec<i32>,
    pub winners: Vec<WinnerSettlementView>,
    #[serde(default)]
    pub played_seats: Vec<u8>,
    #[serde(default)]
    pub confirm_remaining_ms: Option<u64>,
    #[serde(default)]
    pub confirmed_seats: Vec<u8>,
    pub from_seat: Option<u8>,
    #[serde(default)]
    pub ura_dora_indicators: Vec<TileView>,
    /// Impact: the all-in kind that triggered, if any.
    #[serde(default)]
    pub all_in: Option<String>,
    /// Impact: kan-point movement for this hand.
    #[serde(default)]
    pub kan_point_deltas: Option<Vec<i32>>,
    /// Impact: kan-point balances after this hand.
    #[serde(default)]
    pub kan_points_after: Option<Vec<i32>>,
    /// Impact: this hand doesn't count (exhaustive draw).
    #[serde(default)]
    pub void_hand: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct WinnerSettlementView {
    pub seat: u8,
    pub han: u8,
    pub fu: u16,
    pub yakuman_multiplier: u8,
    pub limit: String,
    pub points: u32,
    pub dealer: bool,
    pub yaku: Vec<YakuView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct YakuView {
    pub name: String,
    pub value: u32,
    #[serde(default)]
    pub yakuman: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct ExitVoteView {
    pub initiator_seat: u8,
    pub remaining_ms: u64,
    pub votes: Vec<Option<bool>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProgressView {
    pub round_wind: String,
    pub round_number: u8,
    /// The dealer's seat for this hand (impact uses this directly).
    #[serde(default)]
    pub dealer: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchPhase {
    AwaitingTurnAction { seat: u8 },
    AwaitingDiscard { seat: u8 },
    AwaitingResponses { trigger_seat: u8 },
    Ended { reason: EndReason },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    ExhaustiveDraw,
    NineTerminals,
    FourWinds,
    FourKans,
    FourRiichi,
    Tsumo,
    Ron,
}

/// Reactions available to the observer, from either riichi or impact tables.
///
/// Serde's internal-tag deserialization dispatches on `kind`.  Variants that
/// exist in only one ruleset use `#[serde(alias = …)]` so the bot can derive the
/// right variant without manual string matching.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReactionOptionView {
    // -- riichi --
    Ron,
    Chi {
        tile_ids: [u16; 2],
    },
    Pon {
        tile_ids: [u16; 2],
    },
    OpenKan {
        tile_ids: [u16; 3],
    },
    // -- impact --
    /// Impact pon: the engine picks the two hand tiles; `indicator` means this
    /// pon settles kan points like an open kan would.
    #[serde(alias = "impact_pon")]
    ImpactPon {
        indicator: bool,
    },
    /// Impact open kan: the engine picks the three hand tiles.
    #[serde(alias = "impact_open_kan")]
    ImpactOpenKan,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct MatchPlayerView {
    pub seat: u8,
    #[serde(default)]
    pub nickname: String,
    pub points: i32,
    pub concealed_tiles: Option<Vec<TileView>>,
    pub drawn_tile_id: Option<u16>,
    #[serde(default)]
    pub melds: Vec<MeldView>,
    #[serde(default)]
    pub discards: Vec<DiscardView>,
    #[serde(default)]
    pub riichi_status: String,
    /// Impact: kan-point balance (separate from regular points).
    #[serde(default)]
    pub kan_points: Option<i32>,
    /// Impact: true kans formed this hand (three-kan is an all-in trigger).
    #[serde(default)]
    pub kan_count: Option<u8>,
    /// Impact: consecutive honor/joker discards (11 triggers all-in).
    #[serde(default)]
    pub honor_streak: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TileView {
    pub id: u16,
    pub code: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct MeldView {
    pub kind: String,
    pub tiles: Vec<TileView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscardView {
    pub tile: TileView,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct MatchResultView {
    pub end_reason: String,
    pub hand_count: u32,
    pub placements: Vec<PlacementView>,
    /// Impact: final kan-point balances.
    #[serde(default)]
    pub kan_points: Option<Vec<i32>>,
    /// Impact: point changes since match start.
    #[serde(default)]
    pub point_deltas: Option<Vec<i32>>,
    /// Impact/riichi: final points per seat.
    #[serde(default)]
    pub final_points: Option<Vec<i32>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlacementView {
    pub seat: u8,
    pub rank: u8,
    pub points: i32,
}

/// Pins the wire contract against responses captured from a running server.
#[cfg(test)]
mod tests {
    use super::MatchView;

    const OPENING: &str = include_str!("../fixtures/match_view.json");
    const SETTLEMENT: &str = include_str!("../fixtures/match_view_settlement.json");

    #[test]
    fn parses_a_freshly_dealt_hand() {
        let view: MatchView = serde_json::from_str(OPENING).expect("opening view");
        assert!(view.needs_opening_ready());
        assert!(view.opening_in_progress());
        assert!(view.hand_settlement.is_none());
        assert!(view.observer().is_ok());
    }

    #[test]
    fn parses_a_settlement_before_the_confirm_window_opens() {
        let view: MatchView = serde_json::from_str(SETTLEMENT).expect("settlement view");
        let settlement = view.hand_settlement.as_ref().expect("settlement");
        assert_eq!(settlement.reason, "exhaustive_draw");
        assert!(view.unplayed_settlement().is_some());
        assert!(view.unconfirmed_settlement().is_none());
    }
}
