//! Shared test fixtures for client-side unit and render tests.

use serde_json::{Value, json};

use crate::model::{MatchView, RoomView};
use crate::rules::RuleSetCatalog;

/// Rule-set catalog shaped like `GET /api/v1/rule-sets`.
#[must_use]
pub fn rule_catalog_json() -> Value {
    let ordinary = json!({
        "variant": "yonma",
        "match_rules": {
            "length": "hanchan",
            "initial_points": 25000,
            "return_points": 25000,
            "first_place_required_points": 30000,
            "thinking_time": {"base_seconds": 5, "reserve_seconds": 20},
            "tobi": true,
            "dealer_continuation": "win_or_tenpai",
            "agari_yame": true
        },
        "scoring": {
            "kiriage_mangan": false,
            "old_yaku": false,
            "yakuman_value": "double_variants_and_stacked",
            "nagashi_mangan": true,
            "kazoe_yakuman": true,
            "kokushi_ankan_chankan": true
        },
        "calls": {
            "kuitan": true,
            "kuikae": "forbidden"
        },
        "bonuses": {
            "red_fives": {"man": 1, "pin": 1, "sou": 1},
            "ippatsu": true,
            "ura_dora": true,
            "kan_dora": true
        },
        "abortive_draws": {
            "four_winds": false,
            "four_kans": false,
            "nine_terminals": false,
            "four_riichi": false
        },
        "settlement": {
            "uma": {"type": "fixed", "values": [30, 10, -10, -30]},
            "noten_payment": 3000,
            "ron_resolution": "multiple"
        }
    });
    let mut m_league = ordinary.clone();
    m_league["match_rules"]["tobi"] = json!(false);
    m_league["match_rules"]["agari_yame"] = json!(false);
    m_league["settlement"]["uma"]["values"] = json!([45, 5, -15, -35]);
    m_league["scoring"]["nagashi_mangan"] = json!(false);
    m_league["scoring"]["kazoe_yakuman"] = json!(false);
    m_league["scoring"]["yakuman_value"] = json!("stacked_only");
    m_league["scoring"]["kokushi_ankan_chankan"] = json!(false);
    m_league["settlement"]["ron_resolution"] = json!("head_bump");
    let mut sanma = ordinary.clone();
    sanma["variant"] = json!("sanma");
    sanma["bonuses"]["red_fives"]["man"] = json!(0);
    sanma["settlement"]["uma"]["values"] = json!([30, 0, -30]);

    json!({
        "schema": "rule_set_catalog.v1",
        "rule_sets": [
            {
                "id": "riichi/yonma",
                "display_name": "四人日麻",
                "seat_count": 4,
                "default_config": ordinary,
                "presets": [
                    {
                        "id": "m-league",
                        "revision": 1,
                        "display_name": "M League 规则",
                        "config": m_league
                    }
                ]
            },
            {
                "id": "riichi/sanma",
                "display_name": "三人日麻",
                "seat_count": 3,
                "default_config": sanma,
                "presets": []
            }
        ]
    })
}

/// Parsed catalog for form tests.
///
/// # Panics
/// Panics when the fixture stops matching `rule_set_catalog.v1`.
#[must_use]
pub fn rule_catalog() -> RuleSetCatalog {
    serde_json::from_value(rule_catalog_json()).expect("rule catalog")
}

/// Rule snapshot shaped like the one a room carries.
#[must_use]
pub fn rule_snapshot(rule_set_id: &str) -> Value {
    let catalog = rule_catalog_json();
    let rule_set = catalog["rule_sets"]
        .as_array()
        .and_then(|sets| sets.iter().find(|set| set["id"] == rule_set_id))
        .expect("rule set");
    json!({
        "schema_version": 2,
        "rule_set_id": rule_set_id,
        "engine_version": "riichi-0.1.0",
        "preset": Value::Null,
        "config": rule_set["default_config"].clone()
    })
}

/// Room waiting for players, seat 0 owned by `user-0`.
///
/// # Panics
/// Panics when the fixture stops matching `room_view.v1`.
#[must_use]
pub fn room_view(rule_set_id: &str, seats: u8) -> RoomView {
    let members = (0..seats.min(2))
        .map(|seat| {
            json!({
                "user_id": format!("user-{seat}"),
                "seat": seat,
                "nickname": format!("玩家{}", seat + 1),
                "ready": seat == 0
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "id": "room-1",
        "version": 3,
        "owner_user_id": "user-0",
        "name": "东南战练习房",
        "lifecycle": "waiting",
        "rule_snapshot": rule_snapshot(rule_set_id),
        "members": members,
        "active_match_id": Value::Null
    }))
    .expect("room view")
}

/// Match in progress with the observer on turn, `seats` players.
///
/// # Panics
/// Panics when the fixture stops matching `match_view.v1`.
#[must_use]
pub fn match_view(seats: u8) -> MatchView {
    let players = (0..seats)
        .map(|seat| {
            let concealed = if seat == 0 {
                json!([
                    {"id": 1, "code": "1m"},
                    {"id": 2, "code": "0p"},
                    {"id": 3, "code": "3s"},
                    {"id": 4, "code": "7z"}
                ])
            } else {
                Value::Null
            };
            json!({
                "seat": seat,
                "nickname": format!("玩家{}", seat + 1),
                "points": 25000,
                "concealed_tiles": concealed,
                "concealed_tile_count": if seat == 0 { 4 } else { 13 },
                "drawn_tile_id": if seat == 0 { json!(4) } else { Value::Null },
                "melds": [],
                "discards": [{
                    "tile": {"id": 40, "code": "9s"},
                    "tsumogiri": false,
                    "riichi_declared": false,
                    "claimed_by": Value::Null
                }],
                "riichi_status": "none"
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "id": "match-1",
        "room_id": "room-1",
        "version": 7,
        "event_sequence": 7,
        "hand_index": 0,
        "observer_seat": 0,
        "progress": {
            "round_wind": "east",
            "round_number": 1,
            "dealer": 0,
            "honba": 0,
            "riichi_sticks": 0
        },
        "phase": {"kind": "awaiting_turn_action", "seat": 0},
        "remaining_live_draws": 69,
        "dora_indicators": [{"id": 90, "code": "1z"}],
        "players": players,
        "assets_ready_seats": (0..seats).collect::<Vec<_>>(),
        "available_reactions": [],
        "turn_actions": {
            "can_tsumo": false,
            "riichi_discard_tile_ids": [],
            "concealed_kan_tile_ids": [],
            "added_kan_options": [],
            "can_nine_terminals": false
        },
        "result": Value::Null
    }))
    .expect("match view")
}
