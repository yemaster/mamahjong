//! Application services for identity, rooms, and match orchestration.

mod character;
mod clock;
mod error;
mod game;
mod identity;
mod identity_store;
mod impact_game;
mod matchmaking;
mod music;
mod naming;
mod presentation;
mod record;
mod room;
mod runtime;
mod service;
mod store;
mod stream;
mod tablecloth;

pub use character::{
    Character, CharacterAsset, CharacterOutfit, CharacterVoice, SaveCharacter, VoiceKind,
    action_voices, built_in_action_voices, ichihime_default, yuan_xiao_default,
};
pub use clock::{BASE_THINKING_MS, ClockExpiry, RESERVE_THINKING_MS, SeatClock};
pub use error::{ApplicationError, ErrorCode};
pub use game::{
    AddedKanOption, DiscardWaitHint, GameCommand, GameEventRecord, MatchPlayer, ObserverExitVote,
    ObserverHandSettlement, ObserverMatch, ObserverPlayer, ObserverWinnerSettlement,
    SubmitGameCommand, TurnActions, WaitingTileHint,
};
pub use identity::{
    AccountRole, AccountStatus, CharacterSummary, Nickname, RankSummary, Session, TitleSummary,
    User, UserProfile,
};
pub use impact_game::{
    ImpactDiscardView, ImpactMeldView, ImpactPlayer, ImpactReactionOptionsView, ImpactTileView,
    ImpactTurnActionsView, ObserverImpactExitVote, ObserverImpactMatch, ObserverImpactPlayer,
    ObserverImpactResult, ObserverImpactSettlement, ObserverImpactYaku, ObserverKanPoints,
};
pub use matchmaking::{MatchmakingStatus, MatchmakingTicket};
pub use music::{MusicScene, MusicTrack, SaveMusicTrack, built_in_music_tracks};
pub use naming::{
    end_reason_name, impact_all_in_name, impact_end_reason_name, impact_rule_display_name,
    impact_yaku_name, limit_name, rule_display_name, wind_name, yaku_name,
};
pub use presentation::{
    ACTION_SETTLE_PADDING_MS, ANIMATION_REPORT_GRACE_MS, CALL_BANNER_MS, DISCARD_FLIGHT_MS,
    MATCH_ASSET_LOAD_TIMEOUT_MS, MELD_PUSH_MS, OPENING_READY_FALLBACK_MS, POINTS_REVEAL_MS,
    SETTLEMENT_CONFIRM_MS, SETTLEMENT_COUNTDOWN_MS, SETTLEMENT_FALLBACK_MARGIN_MS,
    SETTLEMENT_REVEAL_BUDGET_MS, animation_grace_ms, discard_animation_ms, meld_call_animation_ms,
    riichi_discard_animation_ms, settlement_fallback_ms, settlement_reveal_fallback_ms,
};
pub use record::MatchRecord;
pub use room::{GameRuleSnapshot, Room, RoomLifecycle, RoomMember, RoomVisibility};
pub use runtime::MatchProjection;
pub use service::{
    Application, CreateRoom, RegisterUser, RoomRuleSelection, UpdateMusic, UpdatePresentation,
    UpdateProfile, UpdateRoom, UpdateTablecloth,
};
pub use stream::{MATCH_EVENT_PAGE_LIMIT, MatchEvent, MatchEventPage};
pub use tablecloth::{SaveTablecloth, Tablecloth, built_in_tablecloths};
