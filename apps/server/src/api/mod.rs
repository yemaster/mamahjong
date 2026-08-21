mod auth;
mod characters;
mod dto;
mod error;
mod identity;
mod matches;
mod matchmaking;
mod music;
mod realtime;
mod records;
mod rooms;
mod rules;
mod tablecloths;

use axum::Router;

use crate::AppState;

pub(crate) use matches::announce_advance;
pub(crate) use realtime::{RealtimeHub, WsTickets, parse_match_stream};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .nest("/api/v1", characters::routes())
        .nest("/api/v1", identity::routes())
        .nest("/api/v1", matchmaking::routes())
        .nest("/api/v1", matches::routes())
        .nest("/api/v1", music::routes())
        .nest("/api/v1", realtime::routes())
        .nest("/api/v1", records::routes())
        .nest("/api/v1", rooms::routes())
        .nest("/api/v1", rules::routes())
        .nest("/api/v1", tablecloths::routes())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use mahjong_riichi::{RiichiVariant, RoomRuleRequest};
    use mamahjong_application::{CreateRoom, RegisterUser, RoomRuleSelection, RoomVisibility};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::{AppState, build_router};

    static NEXT_ARCHIVE: AtomicU64 = AtomicU64::new(0);

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
        let user_id = registration["user"]["id"].as_str().expect("user ID");
        assert_eq!(registration["user"]["profile"]["ranks"], json!([]));
        assert!(registration["user"]["profile"]["equipped_title"].is_null());
        assert!(registration["user"]["profile"]["avatar_path"].is_null());
        assert!(!registration.to_string().contains("correct horse"));

        let (status, presentation) = request_json(
            router.clone(),
            Method::PUT,
            "/api/v1/users/me/presentation",
            Some(token),
            Some(json!({
                "character_id": "ichihime",
                "outfit_id": "beach",
                "avatar_path": "/game/assets/local-characters/mahjong-soul/ichihime/emotes/8.png"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            presentation["profile"]["selected_character"]["id"],
            "ichihime"
        );
        assert_eq!(
            presentation["profile"]["avatar_path"],
            "/game/assets/local-characters/mahjong-soul/ichihime/emotes/8.png"
        );
        assert_eq!(presentation["profile"]["selected_outfit_id"], "beach");

        let (status, detail) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/users/{user_id}/profile"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["schema"], "user_profile_detail.v1");
        assert_eq!(detail["statistics"]["matches_played"], 0);

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
        // 房间顶栏拿它拼「立直麻将 · 四人南 · 自定义规则」——这桌改过头跳。
        assert_eq!(room["rule_name"], "自定义规则");
        assert_eq!(room["members"][0]["nickname"], "玩家owner");
        assert_eq!(room["members"][0]["character"]["id"], "ichihime");
        assert_eq!(
            room["members"][0]["character"]["illustration_path"],
            "/game/assets/local-characters/mahjong-soul/ichihime/outfits/yiji_haitanpaidui.png"
        );
        let room_id = room["id"].as_str().expect("room ID");
        assert_eq!(room_id.len(), 6);
        assert!(room_id.bytes().all(|byte| byte.is_ascii_digit()));
        assert_eq!(room["seat_count"], 4);

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

        let (status, rooms) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/rooms",
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(rooms["schema"], "room_list.v1");
        assert_eq!(rooms["rooms"][0]["id"], room_id);

        let (status, activity) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/users/me/activity",
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(activity["kind"], "room");
        assert_eq!(activity["room_id"], room_id);

        let (status, room) = request_json(
            router.clone(),
            Method::DELETE,
            &format!("/api/v1/rooms/{room_id}/members/me"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(room["lifecycle"], "closed");
        assert_eq!(room["members"].as_array().expect("members").len(), 0);

        let (status, activity) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/users/me/activity",
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(activity["kind"], "idle");

        let (status, error) = request_json(
            router,
            Method::GET,
            &format!("/api/v1/rooms/{room_id}"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["code"], "room.not_found");
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
    async fn music_catalog_lists_both_scenes_and_selection_lands_on_the_profile() {
        let router = build_router(AppState::new());
        let (status, catalog) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/music-tracks",
            None,
            None,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(catalog["schema"], "music_track_list.v1");
        let tracks = catalog["music_tracks"].as_array().expect("music tracks");
        let mut defaults = tracks
            .iter()
            .filter(|track| track["is_default"] == json!(true))
            .map(|track| {
                (
                    track["scene"].as_str().expect("scene"),
                    track["id"].as_str().expect("ID"),
                )
            })
            .collect::<Vec<_>>();
        defaults.sort_unstable();
        // 大厅和对局各有一首默认曲，互不相干。
        assert_eq!(
            defaults,
            vec![("lobby", "lobby-default"), ("match", "zhuqu-zhiyu")]
        );
        assert!(
            tracks
                .iter()
                .all(|track| track["duration_ms"].as_u64().expect("duration") > 0)
        );

        let auth = register(router.clone(), "music").await;
        let token = auth["session"]["token"].as_str().expect("token").to_owned();
        let (status, user) = request_json(
            router.clone(),
            Method::PUT,
            "/api/v1/users/me/music",
            Some(&token),
            Some(json!({"lobby_music_id": "fusheng-touxian"})),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            user["profile"]["selected_lobby_music_id"],
            "fusheng-touxian"
        );
        // 只改了大厅那一项，对局那一项不动。
        assert_eq!(user["profile"]["selected_match_music_id"], Value::Null);

        // 大厅曲不能拿去当对局曲。
        let (status, error) = request_json(
            router,
            Method::PUT,
            "/api/v1/users/me/music",
            Some(&token),
            Some(json!({"match_music_id": "fusheng-touxian"})),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "request.invalid_music_track");
    }

    #[tokio::test]
    async fn rule_catalog_returns_resolved_defaults_and_revisioned_presets() {
        let router = build_router(AppState::new());
        let (status, catalog) =
            request_json(router, Method::GET, "/api/v1/rule-sets", None, None).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(catalog["schema"], "rule_set_catalog.v1");
        let rule_sets = catalog["rule_sets"].as_array().expect("rule sets");
        assert_eq!(rule_sets.len(), 4);

        let yonma = &rule_sets[0];
        assert_eq!(yonma["id"], "riichi/yonma");
        assert_eq!(yonma["family"], "riichi");
        assert_eq!(yonma["seat_count"], 4);
        assert_eq!(
            yonma["default_config"]["settlement"]["uma"]["values"],
            json!([30, 10, -10, -30])
        );
        assert_eq!(
            yonma["presets"]
                .as_array()
                .expect("yonma presets")
                .iter()
                .map(|preset| (
                    preset["id"].as_str().expect("preset ID"),
                    preset["revision"].as_u64().expect("revision")
                ))
                .collect::<Vec<_>>(),
            vec![("jpml-a", 1), ("saikouisen", 1), ("m-league", 1)]
        );

        let sanma = &rule_sets[1];
        assert_eq!(sanma["id"], "riichi/sanma");
        assert_eq!(sanma["family"], "riichi");
        assert_eq!(sanma["seat_count"], 3);
        assert_eq!(sanma["default_config"]["bonuses"]["red_fives"]["man"], 0);
        assert_eq!(sanma["default_config"]["match_rules"]["north"], "nuki_dora");
        assert_eq!(
            sanma["default_config"]["settlement"]["noten_payment"],
            2_000
        );
        assert_eq!(
            sanma["default_config"]["scoring"]["kokushi_ankan_chankan"],
            true
        );
        assert_eq!(sanma["presets"], json!([]));

        // 冲击麻将的默认值就是建房面板打开时该勾上的那一套：杠牌三项全开、
        // 七嵌关、全交九项全开。目录一旦和引擎的 `standard()` 走散，建房页
        // 显示的默认勾选就会和实际开出来的房间不一致。
        let impact = &rule_sets[2];
        assert_eq!(impact["id"], "impact/yonma");
        assert_eq!(impact["family"], "impact");
        assert_eq!(impact["display_name"], "冲击麻将");
        assert_eq!(impact["seat_count"], 4);
        let defaults = &impact["default_config"];
        assert_eq!(defaults["mode"], "blind");
        assert_eq!(defaults["kan"]["added_kan_single_payer"], true);
        assert_eq!(defaults["kan"]["indicator_pon_counts_as_kan"], true);
        assert_eq!(defaults["kan"]["first_round_repeat_discard"], true);
        assert_eq!(defaults["special"]["seven_gaps"], false);
        for toggle in defaults["all_in"].as_object().expect("all-in toggles") {
            assert_eq!(toggle.1, &json!(true), "{} 应当默认开启", toggle.0);
        }
        // 冲击麻将没有流派，目录里不宣告预设，建房页也就不画那个下拉框。
        assert!(
            impact["presets"].as_array().expect("presets").is_empty(),
            "冲击麻将不应当宣告任何预设"
        );

        // 四川麻将的默认值只有思考时间一项，写死为 5 + 20 秒。
        let sichuan = &rule_sets[3];
        assert_eq!(sichuan["id"], "sichuan/yonma");
        assert_eq!(sichuan["family"], "sichuan");
        assert_eq!(sichuan["display_name"], "四川麻将");
        assert_eq!(sichuan["seat_count"], 4);
        assert_eq!(
            sichuan["default_config"]["match_rules"]["thinking_time"],
            json!({"base_seconds": 5, "reserve_seconds": 20})
        );
        // 四川麻将目前只有标准血战到底一份，目录里同样不宣告预设。
        assert!(
            sichuan["presets"].as_array().expect("presets").is_empty(),
            "四川麻将不应当宣告任何预设"
        );
    }

    #[tokio::test]
    async fn sanma_matchmaking_creates_a_playable_match() {
        let router = build_router(AppState::new());
        let mut tokens = Vec::new();
        let mut ticket_ids = Vec::new();
        for index in 0..3 {
            let registration = register(router.clone(), &format!("queue_{index}")).await;
            let token = registration["session"]["token"]
                .as_str()
                .expect("token")
                .to_owned();
            let (status, ticket) = request_json(
                router.clone(),
                Method::POST,
                "/api/v1/matchmaking-tickets",
                Some(&token),
                Some(json!({"rule_set_id": "riichi/sanma"})),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
            tokens.push(token);
            ticket_ids.push(ticket["id"].as_str().expect("ticket ID").to_owned());
        }

        for (token, ticket_id) in tokens.iter().zip(&ticket_ids) {
            let (status, ticket) = request_json(
                router.clone(),
                Method::GET,
                &format!("/api/v1/matchmaking-tickets/{ticket_id}"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(ticket["status"], "matched");
            let match_id = ticket["match_id"].as_str().expect("match ID");
            let (status, view) = request_json(
                router.clone(),
                Method::GET,
                &format!("/api/v1/matches/{match_id}"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(view["players"].as_array().map(Vec::len), Some(3));
            assert!(view["clocks"].is_array());
        }
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
            .start_room(users[0].id(), room.id(), room.version(), state.now_ms())
            .expect("start");
        let router = build_router(state);
        let token = sessions[0].token();

        let (status, activity) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/users/me/activity",
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(activity["kind"], "game");
        assert_eq!(activity["room_id"], room.id().as_str());
        assert_eq!(activity["match_id"], match_id.as_str());

        // 对局素材load完之前服务端一条命令都不收，三家先各报告一次。
        for session in &sessions {
            let (status, _) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/matches/{match_id}/commands"),
                Some(session.token()),
                Some(json!({
                    "expected_version": 0,
                    "command": {"name": "game.assets_ready"}
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }

        // 开局摸牌动画播完之前服务端不收任何对局命令，三家先各报告一次。
        for session in &sessions {
            let (status, _) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/matches/{match_id}/commands"),
                Some(session.token()),
                Some(json!({
                    "expected_version": 0,
                    "command": {
                        "name": "riichi.ready_for_hand",
                        "payload": {"hand_index": 0}
                    }
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }

        let (status, view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let observer_seat = view["observer_seat"].as_u64().expect("observer seat") as usize;
        assert!(view["players"][observer_seat]["concealed_tiles"].is_array());
        for (seat, player) in view["players"]
            .as_array()
            .expect("players")
            .iter()
            .enumerate()
        {
            if seat != observer_seat {
                assert!(player["concealed_tiles"].is_null());
            }
        }
        assert!(view["turn_actions"]["can_tsumo"].is_boolean());
        assert!(view["turn_actions"]["riichi_discard_tile_ids"].is_array());
        assert!(view["turn_actions"]["riichi_discard_hints"].is_array());
        assert!(view["turn_actions"]["concealed_kan_tile_ids"].is_array());
        assert!(view["turn_actions"]["added_kan_options"].is_array());
        assert!(view["turn_actions"]["nuki_tile_ids"].is_array());
        assert_eq!(view["sanma_north_rule"], "nuki_dora");
        assert!(view["players"][observer_seat]["nuki_tiles"].is_array());
        assert!(view["turn_actions"]["can_nine_terminals"].is_boolean());
        assert!(view["players"][observer_seat]["waiting_tiles"].is_array());
        assert!(view["players"][observer_seat]["furiten"].is_boolean());
        let acting_seat = view["phase"]["seat"].as_u64().expect("acting seat") as usize;
        let mut actor_token = None;
        let mut actor_view = None;
        for session in &sessions {
            let (status, candidate) = request_json(
                router.clone(),
                Method::GET,
                &format!("/api/v1/matches/{match_id}"),
                Some(session.token()),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            if candidate["observer_seat"] == acting_seat {
                actor_token = Some(session.token());
                actor_view = Some(candidate);
                break;
            }
        }
        let actor_token = actor_token.expect("acting player token");
        let actor_view = actor_view.expect("acting player view");
        let version_before = actor_view["version"].as_u64().expect("version");
        let tile_id = actor_view["players"][acting_seat]["concealed_tiles"][0]["id"]
            .as_u64()
            .expect("tile ID");

        let (status, discarded) = request_json(
            router,
            Method::POST,
            &format!("/api/v1/matches/{match_id}/commands"),
            Some(actor_token),
            Some(json!({
                "expected_version": actor_view["version"],
                "command": {
                    "name": "riichi.discard",
                    "payload": {"tile_id": tile_id}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(discarded["version"], version_before + 1);
        assert!(discarded["available_reactions"].is_array());
    }

    #[tokio::test]
    async fn complete_sanma_match_can_finish_through_public_http_api() {
        let archive_directory = std::env::temp_dir().join(format!(
            "mamahjong-http-match-test-{}-{}",
            std::process::id(),
            NEXT_ARCHIVE.fetch_add(1, Ordering::Relaxed)
        ));
        let router = build_router(
            AppState::persistent(&archive_directory).expect("persistent application state"),
        );
        let mut tokens = Vec::new();
        for index in 0..3 {
            let registration = register(router.clone(), &format!("full_match_{index}")).await;
            tokens.push(
                registration["session"]["token"]
                    .as_str()
                    .expect("session token")
                    .to_owned(),
            );
        }

        let (status, mut room) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/rooms",
            Some(&tokens[0]),
            Some(json!({
                "name": "完整三麻东风战",
                "visibility": "private",
                "rules": {
                    "rule_set_id": "riichi/sanma",
                    "config": {
                        "overrides": {
                            "match_rules": {
                                "length": "east_only",
                                // 一位必要点数压到起始点数，全场开局就达标，
                                // 东三局打完必然结束，不会南入。见
                                // docs/match-progression.md 第四节。
                                "first_place_required_points": 25000,
                                "tobi": false,
                                "dealer_continuation": "win_only",
                                "agari_yame": false
                            },
                            "scoring": {"nagashi_mangan": false},
                            "abortive_draws": {
                                "four_winds": false,
                                "four_kans": false,
                                "nine_terminals": false,
                                "four_riichi": false
                            }
                        }
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let room_id = room["id"].as_str().expect("room ID").to_owned();

        for token in &tokens[1..] {
            let (status, joined) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/rooms/{room_id}/members"),
                Some(token),
                Some(json!({"expected_version": room["version"]})),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            room = joined;
        }
        for token in &tokens {
            let (status, ready) = request_json(
                router.clone(),
                Method::PUT,
                &format!("/api/v1/rooms/{room_id}/members/me/readiness"),
                Some(token),
                Some(json!({
                    "expected_version": room["version"],
                    "ready": true
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            room = ready;
        }

        let (status, started) = request_json(
            router.clone(),
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/matches"),
            Some(&tokens[0]),
            Some(json!({"expected_version": room["version"]})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let match_id = started["match_id"].as_str().expect("match ID").to_owned();
        let mut seat_tokens = vec![String::new(); tokens.len()];
        for token in &tokens {
            let (status, player_view) = request_json(
                router.clone(),
                Method::GET,
                &format!("/api/v1/matches/{match_id}"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            let seat = player_view["observer_seat"]
                .as_u64()
                .expect("observer seat") as usize;
            seat_tokens[seat] = token.clone();
        }
        // 素材load完之前一步都走不了，先让三家都报到。
        for token in &tokens {
            let (status, _) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/matches/{match_id}/commands"),
                Some(token),
                Some(json!({
                    "expected_version": 0,
                    "command": {"name": "game.assets_ready"}
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, mut view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        for _ in 0..5_000 {
            if !view["result"].is_null() && view["hand_settlement"].is_null() {
                break;
            }
            if view["opening_ready_seats"]
                .as_array()
                .expect("opening ready seats")
                .len()
                < seat_tokens.len()
            {
                // 每一局都要重来一遍：开局摸牌动画播完之前服务端不放行出牌。
                for token in &seat_tokens {
                    let (status, next_view) = request_json(
                        router.clone(),
                        Method::POST,
                        &format!("/api/v1/matches/{match_id}/commands"),
                        Some(token),
                        Some(json!({
                            "expected_version": view["version"],
                            "command": {
                                "name": "riichi.ready_for_hand",
                                "payload": {"hand_index": view["hand_index"]}
                            }
                        })),
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK);
                    view = next_view;
                }
                continue;
            }
            let phase = &view["phase"];
            match phase["kind"].as_str().expect("phase kind") {
                "awaiting_turn_action" | "awaiting_discard" => {
                    let seat = phase["seat"].as_u64().expect("acting seat") as usize;
                    let (status, actor_view) = request_json(
                        router.clone(),
                        Method::GET,
                        &format!("/api/v1/matches/{match_id}"),
                        Some(&seat_tokens[seat]),
                        None,
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK);
                    let tile_id = actor_view["players"][seat]["concealed_tiles"][0]["id"]
                        .as_u64()
                        .expect("own tile ID");
                    let (status, next_view) = request_json(
                        router.clone(),
                        Method::POST,
                        &format!("/api/v1/matches/{match_id}/commands"),
                        Some(&seat_tokens[seat]),
                        Some(json!({
                            "expected_version": actor_view["version"],
                            "command": {
                                "name": "riichi.discard",
                                "payload": {"tile_id": tile_id}
                            }
                        })),
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK);
                    view = next_view;
                }
                "awaiting_responses" => {
                    for token in &tokens {
                        let (status, responder_view) = request_json(
                            router.clone(),
                            Method::GET,
                            &format!("/api/v1/matches/{match_id}"),
                            Some(token),
                            None,
                        )
                        .await;
                        assert_eq!(status, StatusCode::OK);
                        if responder_view["available_reactions"]
                            .as_array()
                            .expect("available reactions")
                            .is_empty()
                        {
                            continue;
                        }
                        let (status, next_view) = request_json(
                            router.clone(),
                            Method::POST,
                            &format!("/api/v1/matches/{match_id}/commands"),
                            Some(token),
                            Some(json!({
                                "expected_version": responder_view["version"],
                                "command": {"name": "riichi.pass"}
                            })),
                        )
                        .await;
                        assert_eq!(status, StatusCode::OK);
                        view = next_view;
                    }
                }
                "ended" => {
                    // 结算是两段：各家先报告动画播完，服务端开了确认窗口，
                    // 才轮得到确认。反过来发确认会被拒。
                    for name in ["riichi.settlement_played", "riichi.confirm_settlement"] {
                        for token in &tokens {
                            let (status, settlement_view) = request_json(
                                router.clone(),
                                Method::GET,
                                &format!("/api/v1/matches/{match_id}"),
                                Some(token),
                                None,
                            )
                            .await;
                            assert_eq!(status, StatusCode::OK);
                            let (status, next_view) = request_json(
                                router.clone(),
                                Method::POST,
                                &format!("/api/v1/matches/{match_id}/commands"),
                                Some(token),
                                Some(json!({
                                    "expected_version": settlement_view["version"],
                                    "command": {
                                        "name": name,
                                        "payload": {
                                            "hand_index": settlement_view["hand_index"]
                                        }
                                    }
                                })),
                            )
                            .await;
                            assert_eq!(status, StatusCode::OK);
                            view = next_view;
                        }
                    }
                }
                phase => panic!("unexpected non-terminal phase: {phase}"),
            }
        }

        let result = &view["result"];
        assert!(!result.is_null(), "match must finish within command bound");
        assert_eq!(view["hand_index"], 2);
        assert_eq!(result["hand_count"], 3);
        assert_eq!(
            result["placements"].as_array().expect("placements").len(),
            3
        );
        assert_eq!(
            result["placements"]
                .as_array()
                .expect("placements")
                .iter()
                .map(|placement| placement["points"].as_i64().expect("points"))
                .sum::<i64>(),
            75_000
        );

        let (status, finished_room) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/rooms/{room_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(finished_room["lifecycle"], "waiting");
        assert!(finished_room["active_match_id"].is_null());
        assert!(
            finished_room["members"]
                .as_array()
                .expect("room members")
                .iter()
                .all(|member| member["ready"] == false)
        );

        let (status, activity) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/users/me/activity",
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(activity["kind"], "room");
        assert_eq!(activity["room_id"], room_id);

        let (status, record) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}/record"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(record["schema"], "match_record.v1");
        // 这局的规则是照着标准规则改出来的，两个接口都得认出「改过」。
        assert_eq!(record["rule_name"], "自定义规则");
        assert_eq!(record["hands"].as_array().expect("hands").len(), 3);
        assert!(
            record["hands"][0]["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
        );
        assert!(!record["result"].is_null());
        // 对局打完了，牌山才跟着牌谱出来——重演要靠它画没人摸到的那些牌。
        assert!(
            record["hands"][0]["wall"]["tiles"]
                .as_array()
                .is_some_and(|tiles| !tiles.is_empty())
        );

        let (status, records) = request_json(
            router.clone(),
            Method::GET,
            "/api/v1/records",
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(records["schema"], "match_record_list.v1");
        let listed = records["records"].as_array().expect("record list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["match_id"], match_id);
        assert_eq!(listed[0]["friend_match"], true);
        assert_eq!(listed[0]["rule_family"], "riichi");
        assert_eq!(listed[0]["variant"], "sanma");
        assert_eq!(listed[0]["match_length"], "east_only");
        assert_eq!(listed[0]["rule_name"], "自定义规则");
        assert_eq!(listed[0]["hand_count"], 3);
        let seats = listed[0]["seats"].as_array().expect("seat rows");
        assert_eq!(seats.len(), 3);
        assert_eq!(seats[0]["rank"], 1);
        // 列表上的增减写的是算过马点的最终得分，三家加起来是零和。
        assert_eq!(
            seats
                .iter()
                .map(|seat| seat["score_tenths"].as_i64().expect("score_tenths"))
                .sum::<i64>(),
            0
        );

        let archived = std::fs::read_to_string(archive_directory.join(format!("{match_id}.json")))
            .expect("durable match record");
        let archived: Value = serde_json::from_str(&archived).expect("archived JSON");
        // `rule_name` 是下发时现算的，不写进归档（预设改版之后存死的名字就成了旧账，
        // 见 docs/match-record-replay.md）。除它以外，两边必须一字不差。
        let mut served = record.clone();
        assert!(
            served
                .as_object_mut()
                .expect("record object")
                .remove("rule_name")
                .is_some()
        );
        assert_eq!(archived, served);
        std::fs::remove_dir_all(archive_directory).expect("remove test archive");
    }

    #[tokio::test]
    async fn impact_friend_room_starts_and_serves_an_impact_match_view() {
        let router = build_router(AppState::new());
        let mut tokens = Vec::new();
        for index in 0..4 {
            let registration = register(router.clone(), &format!("impact_{index}")).await;
            tokens.push(
                registration["session"]["token"]
                    .as_str()
                    .expect("session token")
                    .to_owned(),
            );
        }

        let (status, mut room) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/rooms",
            Some(&tokens[0]),
            Some(json!({
                "name": "冲击麻将好友房",
                "visibility": "private",
                "rules": {
                    "rule_set_id": "impact/yonma",
                    "config": {
                        "overrides": {
                            "special": {"seven_gaps": true},
                            "all_in": {"single_wait": false}
                        }
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(room["variant_kind"], "impact");
        assert_eq!(room["seat_count"], 4);
        // 房间顶栏拿它拼「冲击麻将 · 瞎子麻将 · 自定义规则」。这桌动过两项设置，
        // 所以是自定义；原样开的房这里写「标准规则」，顶栏会把它省掉。
        assert_eq!(room["rule_name"], "自定义规则");
        // 改过的两项照单收下，没改的仍是引擎默认值——建房面板上的勾选和实际开出
        // 来的房间必须一一对应。
        assert_eq!(room["rule_snapshot"]["rule_set_id"], "impact/yonma");
        assert_eq!(
            room["rule_snapshot"]["config"]["special"]["seven_gaps"],
            true
        );
        assert_eq!(
            room["rule_snapshot"]["config"]["all_in"]["single_wait"],
            false
        );
        assert_eq!(
            room["rule_snapshot"]["config"]["all_in"]["three_kans"],
            true
        );
        assert_eq!(
            room["rule_snapshot"]["config"]["kan"]["added_kan_single_payer"],
            true
        );
        let room_id = room["id"].as_str().expect("room ID").to_owned();

        // 立直的配置拿去建冲击房，必须在边界上就被顶回去，而不是带着一半默认值开局。
        let (status, error) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/rooms",
            Some(&tokens[1]),
            Some(json!({
                "name": "拿错配置的房",
                "visibility": "private",
                "rules": {
                    "rule_set_id": "impact/yonma",
                    "config": {"overrides": {"settlement": {"ron_resolution": "head_bump"}}}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "request.invalid_rule_config");

        // 冲击麻将只开好友房，排队入口在参数解析这一层就该拒绝。
        let (status, error) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/matchmaking-tickets",
            Some(&tokens[1]),
            Some(json!({"rule_set_id": "impact/yonma"})),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "request.invalid_rule_set");

        for token in &tokens[1..] {
            let (status, joined) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/rooms/{room_id}/members"),
                Some(token),
                Some(json!({"expected_version": room["version"]})),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            room = joined;
        }
        for token in &tokens {
            let (status, ready) = request_json(
                router.clone(),
                Method::PUT,
                &format!("/api/v1/rooms/{room_id}/members/me/readiness"),
                Some(token),
                Some(json!({
                    "expected_version": room["version"],
                    "ready": true
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            room = ready;
        }

        let (status, started) = request_json(
            router.clone(),
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/matches"),
            Some(&tokens[0]),
            Some(json!({"expected_version": room["version"]})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let match_id = started["match_id"].as_str().expect("match ID").to_owned();

        // 素材与开局动画的握手和立直共用同一组命令，四家都报到之后牌局才动。
        let handshake = [
            json!({"name": "game.assets_ready"}),
            json!({"name": "riichi.ready_for_hand", "payload": {"hand_index": 0}}),
        ];
        for command in &handshake {
            for token in &tokens {
                let (status, _) = request_json(
                    router.clone(),
                    Method::POST,
                    &format!("/api/v1/matches/{match_id}/commands"),
                    Some(token),
                    Some(json!({"expected_version": 0, "command": command})),
                )
                .await;
                assert_eq!(status, StatusCode::OK, "{} 应当被接受", command["name"]);
            }
        }

        let (status, view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["schema"], "match_view.v1");
        assert_eq!(view["variant_kind"], "impact");
        // 左上角只画这一张牌，加一行「连庄 X 次」；宝牌、本场棒、场供棒都不存在。
        assert!(view["joker_indicator"]["code"].is_string());
        assert!(view["joker_code"].is_string());
        assert_eq!(view["dealer_streak"], 0);
        assert_eq!(view["progress"]["honba"], 0);
        assert_eq!(view["progress"]["riichi_sticks"], 0);
        let players = view["players"].as_array().expect("players");
        assert_eq!(players.len(), 4);
        for player in players {
            // 每人 100 点起、杠点 0，界面上写作「100点（0）」。
            assert_eq!(player["points"], 100);
            assert_eq!(player["kan_points"], 0);
        }
        // 庄家摸完第一张就轮到他做事，闲家看不到别人的手牌。
        let seat = view["observer_seat"].as_u64().expect("observer seat");
        let dealer = view["progress"]["dealer"].as_u64().expect("dealer");
        let phase = view["phase"]["kind"].as_str().expect("phase kind");
        if seat == dealer {
            // 起手 14 张里可能就有暗杠或指示牌暗杠，所以是「轮到自己」而不是
            // 「只能打牌」——这一步是打是杠由 `turn_actions` 说了算。
            assert_eq!(phase, "awaiting_turn_action");
            assert_eq!(
                players[seat as usize]["concealed_tiles"]
                    .as_array()
                    .expect("dealer hand")
                    .len(),
                14
            );
        } else {
            assert!(players[seat as usize]["concealed_tiles"].is_array());
            assert!(players[dealer as usize]["concealed_tiles"].is_null());
        }

        // 庄家打一张，证明 `impact.*` 这条命令链一路通到引擎。
        let mut dealer_token = None;
        for token in &tokens {
            let (status, seen) = request_json(
                router.clone(),
                Method::GET,
                &format!("/api/v1/matches/{match_id}"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            if seen["observer_seat"] == seen["progress"]["dealer"] {
                dealer_token = Some((token.clone(), seen));
            }
        }
        let (dealer_token, dealer_view) = dealer_token.expect("庄家的视角");
        let tile_id = dealer_view["players"][dealer as usize]["concealed_tiles"][0]["id"]
            .as_u64()
            .expect("庄家手里的第一张牌");
        let (status, discarded) = request_json(
            router.clone(),
            Method::POST,
            &format!("/api/v1/matches/{match_id}/commands"),
            Some(&dealer_token),
            Some(json!({
                "expected_version": dealer_view["version"],
                "command": {"name": "impact.discard", "payload": {"tile_id": tile_id}}
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(discarded["variant_kind"], "impact");
        assert_eq!(
            discarded["players"][dealer as usize]["discards"]
                .as_array()
                .expect("庄家的牌河")
                .len(),
            1
        );
        // 打完之后要么等人鸣牌，要么直接轮到下家；无论哪种，庄家都不再是行动者。
        assert_ne!(discarded["phase"]["kind"], "awaiting_discard");

        // 冲击麻将不出牌谱，读牌谱要拿到一个明确的错误而不是半张记录，
        // 而开局本身不能因此失败——上面 201 已经证明了这一点。
        let (status, error) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}/record"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "game.invalid_command");
    }

    #[tokio::test]
    async fn sichuan_friend_room_starts_and_serves_a_sichuan_match_view() {
        let router = build_router(AppState::new());
        let mut tokens = Vec::new();
        for index in 0..4 {
            let registration = register(router.clone(), &format!("sichuan_{index}")).await;
            tokens.push(
                registration["session"]["token"]
                    .as_str()
                    .expect("session token")
                    .to_owned(),
            );
        }

        // 四川麻将只开好友房；建房不带 config 就是全默认，标准血战到底。
        let (status, mut room) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/rooms",
            Some(&tokens[0]),
            Some(json!({
                "name": "四川麻将好友房",
                "visibility": "private",
                "rules": {"rule_set_id": "sichuan/yonma"}
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(room["variant_kind"], "sichuan");
        assert_eq!(room["seat_count"], 4);
        // 没动任何选项，顶栏写「标准规则」。
        assert_eq!(room["rule_name"], "标准规则");
        assert_eq!(room["rule_snapshot"]["rule_set_id"], "sichuan/yonma");
        assert_eq!(
            room["rule_snapshot"]["config"]["match_rules"]["thinking_time"],
            json!({"base_seconds": 5, "reserve_seconds": 20})
        );
        let room_id = room["id"].as_str().expect("room ID").to_owned();

        // 立直的配置拿去建川麻房，同样得在边界上被顶回去。
        let (status, error) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/rooms",
            Some(&tokens[1]),
            Some(json!({
                "name": "拿错配置的房",
                "visibility": "private",
                "rules": {
                    "rule_set_id": "sichuan/yonma",
                    "config": {"overrides": {"settlement": {"ron_resolution": "head_bump"}}}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "request.invalid_rule_config");

        // 四川麻将也只开好友房，排队入口在参数解析这一层就拒绝。
        let (status, error) = request_json(
            router.clone(),
            Method::POST,
            "/api/v1/matchmaking-tickets",
            Some(&tokens[1]),
            Some(json!({"rule_set_id": "sichuan/yonma"})),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "request.invalid_rule_set");

        for token in &tokens[1..] {
            let (status, joined) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/rooms/{room_id}/members"),
                Some(token),
                Some(json!({"expected_version": room["version"]})),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            room = joined;
        }
        for token in &tokens {
            let (status, ready) = request_json(
                router.clone(),
                Method::PUT,
                &format!("/api/v1/rooms/{room_id}/members/me/readiness"),
                Some(token),
                Some(json!({
                    "expected_version": room["version"],
                    "ready": true
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            room = ready;
        }

        let (status, started) = request_json(
            router.clone(),
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/matches"),
            Some(&tokens[0]),
            Some(json!({"expected_version": room["version"]})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let match_id = started["match_id"].as_str().expect("match ID").to_owned();

        // 素材与开局动画的握手和立直共用同一组命令，四家都报到之后才进换三张。
        let handshake = [
            json!({"name": "game.assets_ready"}),
            json!({"name": "riichi.ready_for_hand", "payload": {"hand_index": 0}}),
        ];
        for command in &handshake {
            for token in &tokens {
                let (status, _) = request_json(
                    router.clone(),
                    Method::POST,
                    &format!("/api/v1/matches/{match_id}/commands"),
                    Some(token),
                    Some(json!({"expected_version": 0, "command": command})),
                )
                .await;
                assert_eq!(status, StatusCode::OK, "{} 应当被接受", command["name"]);
            }
        }

        let (status, view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["schema"], "match_view.v1");
        assert_eq!(view["variant_kind"], "sichuan");
        // 开局先换三张，方向由骰和决定；定缺与胡牌都还没发生。
        assert_eq!(view["phase"]["kind"], "awaiting_exchange");
        assert!(view["exchange_direction"].is_string());
        assert_eq!(view["exchange_dice"].as_array().map(Vec::len), Some(2));
        let dice_sum = view["exchange_dice"]
            .as_array()
            .expect("exchange dice")
            .iter()
            .map(|die| die.as_u64().expect("die value"))
            .sum::<u64>();
        let expected_direction = match dice_sum {
            2 | 6 | 10 => "counter_clockwise",
            4 | 8 | 12 => "clockwise",
            _ => "opposite",
        };
        assert_eq!(view["exchange_direction"], expected_direction);
        assert!(view["break_seat"].is_u64());
        assert_eq!(view["exchange_submitted_seats"], json!([]));
        assert_eq!(view["dingque_submitted_seats"], json!([]));
        // 没有场风、宝牌、本场棒、连庄；庄家与局数仍要有。
        assert_eq!(view["progress"]["honba"], 0);
        assert_eq!(view["progress"]["riichi_sticks"], 0);
        assert!(view["sichuan_rules"].is_object());
        let players = view["players"].as_array().expect("players");
        assert_eq!(players.len(), 4);
        let observer = view["observer_seat"].as_u64().expect("observer seat");
        let dealer = view["progress"]["dealer"].as_u64().expect("dealer seat");
        for (index, player) in players.iter().enumerate() {
            // 川麻从 0 分起打，杠点不存在，定缺前没有缺门、没人胡牌。
            assert_eq!(player["points"], 0);
            assert!(player["kan_points"].is_null());
            assert_eq!(player["kan_count"], 0);
            assert!(player["que_suit"].is_null());
            assert_eq!(player["won"], false);
            assert!(player["winning_tile"].is_null());
            // 庄家开局已摸第 14 张，换牌时持 14 张；其余各家 13 张。只能看到自己的手牌。
            let seat = player["seat"].as_u64().expect("player seat");
            let expected_tiles = if seat == dealer { 14 } else { 13 };
            assert_eq!(player["concealed_tile_count"], expected_tiles);
            if index as u64 == observer {
                assert_eq!(
                    player["concealed_tiles"]
                        .as_array()
                        .expect("自己的手牌")
                        .len(),
                    expected_tiles
                );
            } else {
                assert!(player["concealed_tiles"].is_null());
            }
        }

        /*
         * 第四家交牌后先进入动画同步阶段。只有四个前端都报告动画播完，服务端
         * 才推进到定缺；定缺花色继续保持私有，直到四家全部提交后才公开。
         */
        let mut exchange_tiles = Vec::new();
        for token in &tokens {
            let (status, own_view) = request_json(
                router.clone(),
                Method::GET,
                &format!("/api/v1/matches/{match_id}"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            let observer = own_view["observer_seat"].as_u64().expect("observer seat");
            let own = own_view["players"]
                .as_array()
                .expect("players")
                .iter()
                .find(|player| player["seat"].as_u64() == Some(observer))
                .expect("observer player");
            let tiles = own["concealed_tiles"].as_array().expect("own tiles");
            let first_suit = ['m', 'p', 's']
                .into_iter()
                .find(|suit| {
                    tiles
                        .iter()
                        .filter(|tile| {
                            tile["code"].as_str().and_then(|code| code.chars().last())
                                == Some(*suit)
                        })
                        .count()
                        >= 3
                })
                .expect("a hand has three tiles of one suit");
            let selected = tiles
                .iter()
                .filter(|tile| {
                    tile["code"].as_str().and_then(|code| code.chars().last()) == Some(first_suit)
                })
                .take(3)
                .map(|tile| tile["id"].as_u64().expect("tile id") as u16)
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 3);
            exchange_tiles.push(selected);
        }
        let mut expected_version = view["version"].as_u64().expect("match version");
        for (token, selected) in tokens.iter().zip(exchange_tiles) {
            let (status, response) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/matches/{match_id}/commands"),
                Some(token),
                Some(json!({
                    "expected_version": expected_version,
                    "command": {"name": "sichuan.exchange", "payload": {"tile_ids": selected}}
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "exchange response: {response}");
            expected_version = response["version"].as_u64().expect("match version");
        }
        let (status, animation_view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            animation_view["phase"]["kind"],
            "awaiting_exchange_animation"
        );
        assert_eq!(animation_view["exchange_animation_played_seats"], json!([]));

        for token in &tokens {
            let (status, response) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/matches/{match_id}/commands"),
                Some(token),
                Some(json!({
                    "expected_version": expected_version,
                    "command": {"name": "sichuan.exchange_animation_played"}
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            expected_version = response["version"].as_u64().expect("match version");
        }

        let (status, dingque_view) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(dingque_view["phase"]["kind"], "awaiting_dingque");
        assert_eq!(dingque_view["dingque_submitted_seats"], json!([]));
        for player in dingque_view["players"].as_array().expect("players") {
            assert!(player["que_suit"].is_null());
        }

        for token in &tokens {
            let (status, _) = request_json(
                router.clone(),
                Method::POST,
                &format!("/api/v1/matches/{match_id}/commands"),
                Some(token),
                Some(json!({
                    "expected_version": expected_version,
                    "command": {"name": "sichuan.ding_que", "payload": {"suit": "man"}}
                })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            expected_version += 1;
        }
        let (status, after_dingque) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(after_dingque["phase"]["kind"] != "awaiting_dingque");
        assert_eq!(
            after_dingque["dingque_submitted_seats"],
            json!([0, 1, 2, 3])
        );
        for player in after_dingque["players"].as_array().expect("players") {
            assert_eq!(player["que_suit"], "man");
        }

        // 四川麻将不出牌谱，读牌谱拿明确错误，开局本身不受影响。
        let (status, error) = request_json(
            router.clone(),
            Method::GET,
            &format!("/api/v1/matches/{match_id}/record"),
            Some(&tokens[0]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "game.invalid_command");
    }
}
