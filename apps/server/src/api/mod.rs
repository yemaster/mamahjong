mod auth;
mod dto;
mod error;
mod identity;
mod matches;
mod rooms;

use axum::Router;

use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .nest("/api/v1", identity::routes())
        .nest("/api/v1", matches::routes())
        .nest("/api/v1", rooms::routes())
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
    use mamahjong_application::{CreateRoom, RegisterUser, RoomRuleSelection, RoomVisibility};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::{AppState, build_router};

    async fn request_json(
        router: Router,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(&value).expect("encode JSON"))
            }
            None => Body::empty(),
        };
        let response = router
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1_048_576)
            .await
            .expect("response body");
        let value = serde_json::from_slice(&bytes).expect("JSON response");
        (status, value)
    }

    async fn register(router: Router, suffix: &str) -> Value {
        let (status, response) = request_json(
            router,
            Method::POST,
            "/api/v1/registrations",
            None,
            Some(json!({
                "login_name": format!("api_{suffix}"),
                "password": "correct horse battery staple",
                "nickname": format!("玩家{suffix}")
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        response
    }

    #[tokio::test]
    async fn registration_authentication_and_room_flow_use_stable_json() {
        let router = build_router(AppState::new());
        let registration = register(router.clone(), "owner").await;
        let token = registration["session"]["token"].as_str().expect("token");
        assert_eq!(registration["user"]["profile"]["ranks"], json!([]));
        assert!(registration["user"]["profile"]["equipped_title"].is_null());
        assert!(!registration.to_string().contains("correct horse"));

        let (status, error) =
            request_json(router.clone(), Method::GET, "/api/v1/users/me", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error["code"], "auth.missing_bearer");

        let (status, room) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/rooms",
            Some(token),
            Some(json!({
                "name": "API 头跳房",
                "visibility": "public",
                "rules": {
                    "rule_set_id": "riichi/yonma",
                    "config": {
                        "overrides": {
                            "settlement": {"ron_resolution": "head_bump"}
                        }
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            room["rule_snapshot"]["config"]["settlement"]["ron_resolution"],
            "head_bump"
        );
        assert_eq!(room["members"][0]["nickname"], "玩家owner");
        let room_id = room["id"].as_str().expect("room ID");

        let (status, stale) = request_json(
            router.clone(),
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/members"),
            Some(token),
            Some(json!({"expected_version": 0})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(stale["code"], "room.version_conflict");

        let (status, rooms) =
            request_json(router, Method::GET, "/api/v1/rooms", Some(token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(rooms["schema"], "room_list.v1");
        assert_eq!(rooms["rooms"][0]["id"], room_id);
    }

    #[tokio::test]
    async fn unknown_json_fields_are_rejected_with_api_error_envelope() {
        let router = build_router(AppState::new());
        let (status, error) = request_json(
            router,
            Method::POST,
            "/api/v1/registrations",
            None,
            Some(json!({
                "login_name": "unknown_field",
                "password": "correct horse battery staple",
                "nickname": "测试用户",
                "admin": true
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["kind"], "error");
        assert_eq!(error["schema"], "error.v1");
        assert_eq!(error["code"], "request.invalid_json");
    }

    #[tokio::test]
    async fn match_view_hides_opponents_and_accepts_versioned_commands() {
        let state = AppState::new();
        let mut users = Vec::new();
        let mut sessions = Vec::new();
        for index in 0..3 {
            let (user, session) = state
                .application()
                .register(RegisterUser {
                    login_name: format!("transport_player_{index}"),
                    password: "correct horse battery staple".to_owned(),
                    nickname: format!("玩家{index}"),
                })
                .expect("register");
            users.push(user);
            sessions.push(session);
        }
        let room = state
            .application()
            .create_room(
                users[0].id(),
                CreateRoom {
                    name: "传输测试三麻".to_owned(),
                    visibility: RoomVisibility::Private,
                    rules: RoomRuleSelection::Riichi {
                        variant: RiichiVariant::Sanma,
                        request: RoomRuleRequest::default(),
                    },
                },
            )
            .expect("room");
        let room = state
            .application()
            .join_room(users[1].id(), room.id(), room.version())
            .expect("join");
        let room = state
            .application()
            .join_room(users[2].id(), room.id(), room.version())
            .expect("join");
        let mut room = state
            .application()
            .set_ready(users[0].id(), room.id(), room.version(), true)
            .expect("ready");
        for user in &users[1..] {
            room = state
                .application()
                .set_ready(user.id(), room.id(), room.version(), true)
                .expect("ready");
        }
        let (_, match_id) = state
            .application()
            .start_room(users[0].id(), room.id(), room.version())
            .expect("start");
        let router = build_router(state);
        let token = sessions[0].token();

        let (status, view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["observer_seat"], 0);
        assert!(view["players"][0]["concealed_tiles"].is_array());
        assert!(view["players"][1]["concealed_tiles"].is_null());
        let tile_id = view["players"][0]["concealed_tiles"][0]["id"]
            .as_u64()
            .expect("tile ID");

        let (status, discarded) = request_json(
            router,
            Method::POST,
            &format!("/api/v1/matches/{match_id}/commands"),
            Some(token),
            Some(json!({
                "expected_version": view["version"],
                "command": {
                    "name": "riichi.discard",
                    "payload": {"tile_id": tile_id}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(discarded["version"], 2);
        assert_eq!(discarded["phase"]["kind"], "awaiting_responses");
    }
}
