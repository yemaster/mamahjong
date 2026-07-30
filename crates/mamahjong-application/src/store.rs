use std::collections::HashMap;

use mahjong_core::{RoomId, UserId};

use crate::{Room, Session, User};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub(crate) users: HashMap<UserId, User>,
    pub(crate) login_index: HashMap<String, UserId>,
    pub(crate) password_hashes: HashMap<UserId, String>,
    pub(crate) sessions: HashMap<String, Session>,
    pub(crate) rooms: HashMap<RoomId, Room>,
}
