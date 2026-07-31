use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct AuthResponse {
    pub user: UserView,
    pub session: SessionView,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UserView {
    pub id: String,
    pub profile: ProfileView,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProfileView {
    pub nickname: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionView {
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomList {
    pub rooms: Vec<RoomView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomView {
    pub id: String,
    pub version: u64,
    pub owner_user_id: String,
    pub name: String,
    pub lifecycle: String,
    pub rule_snapshot: Value,
    pub members: Vec<RoomMemberView>,
    pub active_match_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomMemberView {
    pub user_id: String,
    pub seat: u8,
    pub nickname: String,
    pub ready: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StartMatchResponse {
    pub match_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchView {
    pub id: String,
    pub version: u64,
    pub event_sequence: u64,
    pub hand_index: u32,
    pub observer_seat: u8,
    pub progress: ProgressView,
    pub phase: MatchPhase,
    pub remaining_live_draws: usize,
    pub dora_indicators: Vec<TileView>,
    pub players: Vec<MatchPlayerView>,
    pub result: Option<MatchResultView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProgressView {
    pub round_wind: String,
    pub round_number: u8,
    pub dealer: u8,
    pub honba: u32,
    pub riichi_sticks: u32,
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

#[derive(Clone, Debug, Deserialize)]
pub struct MatchPlayerView {
    pub seat: u8,
    pub nickname: String,
    pub points: i32,
    pub concealed_tiles: Option<Vec<TileView>>,
    pub concealed_tile_count: usize,
    pub drawn_tile_id: Option<u16>,
    pub melds: Vec<MeldView>,
    pub discards: Vec<DiscardView>,
    pub riichi_status: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TileView {
    pub id: u16,
    pub code: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MeldView {
    pub kind: String,
    pub tiles: Vec<TileView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscardView {
    pub tile: TileView,
    pub tsumogiri: bool,
    pub riichi_declared: bool,
    pub claimed_by: Option<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchResultView {
    pub end_reason: String,
    pub placements: Vec<PlacementView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlacementView {
    pub seat: u8,
    pub rank: u8,
    pub points: i32,
    pub score_tenths: i32,
}

#[derive(Clone, Debug)]
pub struct ApiFailure {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiFailure {}
