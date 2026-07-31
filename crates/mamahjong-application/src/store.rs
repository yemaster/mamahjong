use std::collections::HashMap;

use mahjong_core::{MatchId, RoomId, TicketId, UserId};

use crate::game::GameRuntime;
use crate::{MatchmakingTicket, Room, Session, User};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub(crate) users: HashMap<UserId, User>,
    pub(crate) login_index: HashMap<String, UserId>,
    pub(crate) password_hashes: HashMap<UserId, String>,
    pub(crate) sessions: HashMap<String, Session>,
    pub(crate) rooms: HashMap<RoomId, Room>,
    pub(crate) matches: HashMap<MatchId, GameRuntime>,
    pub(crate) matchmaking_tickets: HashMap<TicketId, MatchmakingTicket>,
    pub(crate) next_matchmaking_order: u64,
}
