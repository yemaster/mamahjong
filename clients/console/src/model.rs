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
pub struct MatchmakingTicketView {
    pub id: String,
    pub rule_set_id: String,
    pub status: String,
    pub match_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchView {
    pub id: String,
    pub room_id: String,
    pub version: u64,
    pub event_sequence: u64,
    pub hand_index: u32,
    pub observer_seat: u8,
    pub progress: ProgressView,
    pub phase: MatchPhase,
    pub remaining_live_draws: usize,
    pub dora_indicators: Vec<TileView>,
    pub players: Vec<MatchPlayerView>,
    pub available_reactions: Vec<ReactionOptionView>,
    pub turn_actions: TurnActionsView,
    /// 已经把对局素材load完的座位。全场报到之前服务端一条命令都不收。
    #[serde(default)]
    pub assets_ready_seats: Vec<u8>,
    /// 有人一直没load完，这局已经作废。
    #[serde(default)]
    pub terminated_by_asset_timeout: bool,
    pub result: Option<MatchResultView>,
}

impl MatchView {
    /// 本座还欠服务端一次素材load完的报告。
    pub fn needs_assets_ready(&self) -> bool {
        !self.assets_ready_seats.contains(&self.observer_seat)
    }

    /// 还有人没load完，桌面是冻着的。
    pub fn assets_loading(&self) -> bool {
        self.assets_ready_seats.len() < self.players.len()
    }
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
pub struct TurnActionsView {
    pub can_tsumo: bool,
    pub riichi_discard_tile_ids: Vec<u16>,
    pub concealed_kan_tile_ids: Vec<[u16; 4]>,
    pub added_kan_options: Vec<AddedKanOptionView>,
    pub can_nine_terminals: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub struct AddedKanOptionView {
    pub meld_id: u8,
    pub tile_id: u16,
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
        let message = match self.code.as_str() {
            "client.transport" => "无法连接服务器",
            "client.invalid_response" => "服务器响应异常",
            "request.invalid_login_name" => "登录名格式不正确",
            "request.invalid_password" => "密码需为 10 至 128 个字符",
            "request.invalid_nickname" => "昵称格式不正确",
            "request.invalid_room_name" => "房间名格式不正确",
            "request.invalid_rule_config" => "规则设置有误",
            "request.invalid_json" => "请求格式有误",
            "auth.login_name_taken" => "登录名已被使用",
            "auth.invalid_credentials" => "登录名或密码错误",
            "auth.invalid_session" | "auth.missing_bearer" => "登录已失效",
            "auth.user_unavailable" => "账号当前不可用",
            "room.not_found" | "room.closed" => "房间已关闭",
            "room.full" => "房间已满",
            "room.already_member" => "你已在该房间",
            "room.not_member" => "你不在该房间",
            "room.not_owner" => "仅房主可操作",
            "room.version_conflict" => "房间状态已更新，请重试",
            "room.playing" => "对局已经开始",
            "room.not_ready" => "人数未满或有人未准备",
            "game.not_found" => "对局不存在",
            "game.not_player" => "你不是本局玩家",
            "game.stale_version" => "牌局状态已更新，请重试",
            "game.invalid_command" => "当前不能执行此操作",
            "game.finished" => "对局已经结束",
            "request.invalid_rule_set" => "该匹配玩法不可用",
            "matchmaking.already_queued" => "你已经在匹配队列中",
            "matchmaking.ticket_not_found" => "匹配票不存在",
            "matchmaking.ticket_not_waiting" => "匹配已经结束，不能取消",
            "lobby.user_busy" => "你已在其他房间或匹配队列中",
            "server.internal" | "server.unknown" => "服务器暂时不可用",
            "client.invalid_input" => self.message.as_str(),
            _ => "操作失败",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ApiFailure {}

#[cfg(test)]
mod tests {
    use super::ReactionOptionView;

    #[test]
    fn reaction_options_decode_exact_server_tile_ids() {
        let option = serde_json::from_value::<ReactionOptionView>(serde_json::json!({
            "kind": "pon",
            "tile_ids": [17, 42]
        }))
        .expect("reaction option");

        assert_eq!(option, ReactionOptionView::Pon { tile_ids: [17, 42] });
    }
}
