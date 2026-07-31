use mahjong_core::{MatchId, RoomId, TicketId, UserId};
use mahjong_riichi::RiichiVariant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchmakingStatus {
    Waiting,
    Matched { room_id: RoomId, match_id: MatchId },
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchmakingTicket {
    id: TicketId,
    user_id: UserId,
    variant: RiichiVariant,
    status: MatchmakingStatus,
    pub(crate) join_order: u64,
}

impl MatchmakingTicket {
    pub(crate) fn new(user_id: UserId, variant: RiichiVariant, join_order: u64) -> Self {
        Self {
            id: TicketId::new(),
            user_id,
            variant,
            status: MatchmakingStatus::Waiting,
            join_order,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &TicketId {
        &self.id
    }

    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub const fn variant(&self) -> RiichiVariant {
        self.variant
    }

    #[must_use]
    pub const fn status(&self) -> &MatchmakingStatus {
        &self.status
    }

    pub(crate) fn mark_matched(&mut self, room_id: RoomId, match_id: MatchId) {
        self.status = MatchmakingStatus::Matched { room_id, match_id };
    }

    pub(crate) fn cancel(&mut self) {
        self.status = MatchmakingStatus::Cancelled;
    }
}
