//! Fixtures shared by the server's own tests.

use mahjong_core::MatchId;
use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
use mamahjong_application::{
    CreateRoom, GameCommand, RegisterUser, RoomRuleSelection, RoomVisibility, Session,
    SubmitGameCommand, User,
};

use crate::AppState;

/// Registers `count` players; the first three are seated by [`sanma_match`].
pub(crate) fn players(state: &AppState, suffix: &str, count: usize) -> Vec<(User, Session)> {
    (0..count)
        .map(|index| {
            state
                .application()
                .register(RegisterUser {
                    login_name: format!("player_{suffix}_{index}"),
                    password: "correct horse battery staple".to_owned(),
                    nickname: format!("玩家{index}"),
                })
                .expect("register")
        })
        .collect()
}

/// Seats the first three players at a started three-player table.
pub(crate) fn sanma_match(state: &AppState, seated: &[User]) -> MatchId {
    let application = state.application();
    let mut room = application
        .create_room(
            seated[0].id(),
            CreateRoom {
                name: "三麻".to_owned(),
                visibility: RoomVisibility::Private,
                rules: RoomRuleSelection::Riichi {
                    variant: RiichiVariant::Sanma,
                    request: RoomRuleRequest::default(),
                },
            },
        )
        .expect("room");
    for user in &seated[1..] {
        room = application
            .join_room(user.id(), room.id(), room.version())
            .expect("join");
    }
    for user in seated {
        room = application
            .set_ready(user.id(), room.id(), room.version(), true)
            .expect("ready");
    }
    let (_, match_id) = application
        .start_room(seated[0].id(), room.id(), room.version(), state.now_ms())
        .expect("start");
    // 素材load完之前服务端一步都不走，先让各家报到。
    for user in seated {
        application
            .submit_game_command(
                user.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: 0,
                    command: GameCommand::MatchAssetsReady,
                },
                state.now_ms(),
            )
            .expect("assets ready");
    }
    for user in seated {
        application
            .submit_game_command(
                user.id(),
                &match_id,
                SubmitGameCommand {
                    expected_version: 0,
                    command: GameCommand::ReadyForHand { hand_index: 0 },
                },
                state.now_ms(),
            )
            .expect("opening ready");
    }
    match_id
}
