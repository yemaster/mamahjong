use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct AuthResponse {
    pub user: UserView,
    pub session: SessionView,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UserView {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionView {
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomView {
    pub id: String,
    pub version: u64,
    pub members: Vec<RoomMemberView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomMemberView {
    pub user_id: String,
    pub seat: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StartMatchResponse {
    pub match_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchView {
    pub version: u64,
    pub hand_index: u32,
    pub observer_seat: u8,
    pub progress: ProgressView,
    pub phase: MatchPhase,
    pub dora_indicators: Vec<TileView>,
    pub players: Vec<MatchPlayerView>,
    pub available_reactions: Vec<ReactionOptionView>,
    pub result: Option<MatchResultView>,
}

impl MatchView {
    pub fn observer(&self) -> Result<&MatchPlayerView, String> {
        self.players
            .iter()
            .find(|player| player.seat == self.observer_seat)
            .ok_or_else(|| "观察者座位不存在".to_owned())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProgressView {
    pub round_wind: String,
    pub round_number: u8,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReactionOptionView {
    Ron,
    Chi { tile_ids: [u16; 2] },
    Pon { tile_ids: [u16; 2] },
    OpenKan { tile_ids: [u16; 3] },
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchPlayerView {
    pub seat: u8,
    pub nickname: String,
    pub points: i32,
    pub concealed_tiles: Option<Vec<TileView>>,
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
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchResultView {
    pub end_reason: String,
    pub hand_count: u32,
    pub placements: Vec<PlacementView>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlacementView {
    pub seat: u8,
    pub rank: u8,
    pub points: i32,
}
