use std::collections::HashMap;

use mahjong_core::{MatchId, RoomId, TicketId, UserId};

use crate::runtime::GameRuntime;
use crate::{Character, MatchmakingTicket, MusicTrack, Room, Session, Tablecloth, User};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub(crate) users: HashMap<UserId, User>,
    pub(crate) characters: HashMap<String, Character>,
    pub(crate) tablecloths: HashMap<String, Tablecloth>,
    pub(crate) music_tracks: HashMap<String, MusicTrack>,
    pub(crate) login_index: HashMap<String, UserId>,
    pub(crate) password_hashes: HashMap<UserId, String>,
    pub(crate) sessions: HashMap<String, Session>,
    pub(crate) rooms: HashMap<RoomId, Room>,
    pub(crate) matches: HashMap<MatchId, GameRuntime>,
    pub(crate) matchmaking_tickets: HashMap<TicketId, MatchmakingTicket>,
    pub(crate) next_matchmaking_order: u64,
}
