use mahjong_core::{MatchId, RoomId, UserId};
use mahjong_riichi::{RiichiRuleSnapshot, RiichiVariant};

use crate::{ApplicationError, ErrorCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomVisibility {
    Public,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomLifecycle {
    Waiting,
    Playing,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameRuleSnapshot {
    Riichi(RiichiRuleSnapshot),
}

impl GameRuleSnapshot {
    #[must_use]
    pub const fn seat_count(&self) -> u8 {
        match self {
            Self::Riichi(snapshot) => snapshot.rules().variant.seat_count().value(),
        }
    }

    #[must_use]
    pub const fn riichi_variant(&self) -> Option<RiichiVariant> {
        match self {
            Self::Riichi(snapshot) => Some(snapshot.rules().variant),
        }
    }

    #[must_use]
    pub const fn as_riichi(&self) -> Option<&RiichiRuleSnapshot> {
        match self {
            Self::Riichi(snapshot) => Some(snapshot),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomMember {
    user_id: UserId,
    seat: u8,
    nickname: String,
    ready: bool,
    join_order: u64,
}

impl RoomMember {
    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub const fn seat(&self) -> u8 {
        self.seat
    }

    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }

    #[must_use]
    pub const fn ready(&self) -> bool {
        self.ready
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Room {
    id: RoomId,
    version: u64,
    owner_user_id: UserId,
    name: String,
    visibility: RoomVisibility,
    lifecycle: RoomLifecycle,
    rule_snapshot: GameRuleSnapshot,
    members: Vec<RoomMember>,
    next_join_order: u64,
    active_match_id: Option<MatchId>,
}

impl Room {
    pub(crate) fn new(
        owner_user_id: UserId,
        owner_nickname: String,
        name: String,
        visibility: RoomVisibility,
        rule_snapshot: GameRuleSnapshot,
    ) -> Self {
        let owner = RoomMember {
            user_id: owner_user_id.clone(),
            seat: 0,
            nickname: owner_nickname,
            ready: false,
            join_order: 0,
        };
        Self {
            id: RoomId::new(),
            version: 1,
            owner_user_id,
            name,
            visibility,
            lifecycle: RoomLifecycle::Waiting,
            rule_snapshot,
            members: vec![owner],
            next_join_order: 1,
            active_match_id: None,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &RoomId {
        &self.id
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub const fn owner_user_id(&self) -> &UserId {
        &self.owner_user_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn visibility(&self) -> RoomVisibility {
        self.visibility
    }

    #[must_use]
    pub const fn lifecycle(&self) -> RoomLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn rule_snapshot(&self) -> &GameRuleSnapshot {
        &self.rule_snapshot
    }

    #[must_use]
    pub fn members(&self) -> &[RoomMember] {
        &self.members
    }

    #[must_use]
    pub const fn active_match_id(&self) -> Option<&MatchId> {
        self.active_match_id.as_ref()
    }

    pub(crate) fn join(
        &mut self,
        user_id: UserId,
        nickname: String,
    ) -> Result<(), ApplicationError> {
        self.ensure_waiting()?;
        if self.members.iter().any(|member| member.user_id == user_id) {
            return Err(ApplicationError::new(
                ErrorCode::AlreadyRoomMember,
                "user is already a room member",
            ));
        }
        let seat_count = self.rule_snapshot.seat_count();
        let Some(seat) =
            (0..seat_count).find(|seat| self.members.iter().all(|member| member.seat != *seat))
        else {
            return Err(ApplicationError::new(ErrorCode::RoomFull, "room is full"));
        };
        self.members.push(RoomMember {
            user_id,
            seat,
            nickname,
            ready: false,
            join_order: self.next_join_order,
        });
        self.next_join_order += 1;
        self.version += 1;
        Ok(())
    }

    pub(crate) fn set_ready(
        &mut self,
        user_id: &UserId,
        ready: bool,
    ) -> Result<(), ApplicationError> {
        self.ensure_waiting()?;
        let member = self
            .members
            .iter_mut()
            .find(|member| &member.user_id == user_id)
            .ok_or_else(|| {
                ApplicationError::new(ErrorCode::NotRoomMember, "user is not a room member")
            })?;
        if member.ready != ready {
            member.ready = ready;
            self.version += 1;
        }
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        actor: &UserId,
        name: Option<String>,
        visibility: Option<RoomVisibility>,
        rule_snapshot: Option<GameRuleSnapshot>,
    ) -> Result<(), ApplicationError> {
        self.ensure_owner(actor)?;
        self.ensure_waiting()?;
        if let Some(snapshot) = &rule_snapshot {
            if usize::from(snapshot.seat_count()) < self.members.len() {
                return Err(ApplicationError::new(
                    ErrorCode::RoomFull,
                    "new rules have fewer seats than current members",
                ));
            }
        }
        if let Some(name) = name {
            self.name = name;
        }
        if let Some(visibility) = visibility {
            self.visibility = visibility;
        }
        if let Some(snapshot) = rule_snapshot {
            self.rule_snapshot = snapshot;
            for member in &mut self.members {
                member.ready = false;
            }
        }
        self.version += 1;
        Ok(())
    }

    pub(crate) fn leave(&mut self, user_id: &UserId) -> Result<(), ApplicationError> {
        self.ensure_waiting()?;
        let Some(index) = self
            .members
            .iter()
            .position(|member| &member.user_id == user_id)
        else {
            return Err(ApplicationError::new(
                ErrorCode::NotRoomMember,
                "user is not a room member",
            ));
        };
        let owner_left = self.owner_user_id == *user_id;
        self.members.remove(index);
        if self.members.is_empty() {
            self.lifecycle = RoomLifecycle::Closed;
        } else if owner_left {
            let successor = self
                .members
                .iter()
                .min_by_key(|member| member.join_order)
                .expect("non-empty members have a successor");
            self.owner_user_id = successor.user_id.clone();
        }
        self.version += 1;
        Ok(())
    }

    pub(crate) fn start(&mut self, actor: &UserId) -> Result<MatchId, ApplicationError> {
        self.ensure_owner(actor)?;
        self.ensure_waiting()?;
        if self.members.len() != usize::from(self.rule_snapshot.seat_count())
            || self.members.iter().any(|member| !member.ready)
        {
            return Err(ApplicationError::new(
                ErrorCode::RoomNotReady,
                "all seats must be occupied and ready",
            ));
        }
        let match_id = MatchId::new();
        self.lifecycle = RoomLifecycle::Playing;
        self.active_match_id = Some(match_id.clone());
        self.version += 1;
        Ok(match_id)
    }

    fn ensure_waiting(&self) -> Result<(), ApplicationError> {
        match self.lifecycle {
            RoomLifecycle::Waiting => Ok(()),
            RoomLifecycle::Playing => Err(ApplicationError::new(
                ErrorCode::RoomPlaying,
                "room is currently playing",
            )),
            RoomLifecycle::Closed => Err(ApplicationError::new(
                ErrorCode::RoomClosed,
                "room is closed",
            )),
        }
    }

    fn ensure_owner(&self, actor: &UserId) -> Result<(), ApplicationError> {
        if self.owner_user_id == *actor {
            Ok(())
        } else {
            Err(ApplicationError::new(
                ErrorCode::NotRoomOwner,
                "only the room owner may perform this action",
            ))
        }
    }
}
