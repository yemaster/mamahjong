//! 观察者视图的结构化差分。
//!
//! 一份中盘视图约 7 KB，其中八成五是 `players`，而每次推进真正变的只有几个
//! 字段。这里把两份视图逐层比较，只留下不一样的地方；完全没变的子树一层层
//! 向上省略掉。
//!
//! 这套差分不认识麻将：它只看 JSON 的形状。规则引擎算出来的那些字段
//! （`turn_actions`、`waiting_tiles`、振听……）因此不必在传输层重新推导，
//! 客户端也不用照着事件类型分支。

use serde_json::{Map, Value};

/// 求出从 `previous` 到 `next` 的补丁；两者相同时返回 `None`（一帧都不必发）。
pub(super) fn diff(previous: &Value, next: &Value) -> Option<Value> {
    if previous == next {
        return None;
    }
    match (previous, next) {
        (Value::Object(before), Value::Object(after)) => diff_object(before, after),
        (Value::Array(before), Value::Array(after)) => diff_array(before, after),
        _ => Some(replace(next)),
    }
}

fn replace(value: &Value) -> Value {
    let mut op = Map::new();
    op.insert("set".to_owned(), value.clone());
    Value::Object(op)
}

fn diff_object(before: &Map<String, Value>, after: &Map<String, Value>) -> Option<Value> {
    let mut changed = Map::new();
    for (key, next) in after {
        match before.get(key) {
            Some(previous) => {
                if let Some(patch) = diff(previous, next) {
                    changed.insert(key.clone(), patch);
                }
            }
            None => {
                changed.insert(key.clone(), replace(next));
            }
        }
    }
    let removed: Vec<Value> = before
        .keys()
        .filter(|key| !after.contains_key(*key))
        .map(|key| Value::String(key.clone()))
        .collect();
    if changed.is_empty() && removed.is_empty() {
        return None;
    }
    let mut op = Map::new();
    if !changed.is_empty() {
        op.insert("obj".to_owned(), Value::Object(changed));
    }
    if !removed.is_empty() {
        op.insert("del".to_owned(), Value::Array(removed));
    }
    Some(Value::Object(op))
}

/// 数组按下标对齐：公共部分逐个求差，长出来的部分整段追加，短了的由 `len` 截断。
///
/// 牌河加一张牌因此只发一个 `push`，前面那些牌一个字节都不重发。
fn diff_array(before: &[Value], after: &[Value]) -> Option<Value> {
    let common = before.len().min(after.len());
    let mut at = Map::new();
    for index in 0..common {
        if let Some(patch) = diff(&before[index], &after[index]) {
            at.insert(index.to_string(), patch);
        }
    }
    let appended = &after[common..];
    let truncated = after.len() < before.len();
    if at.is_empty() && appended.is_empty() && !truncated {
        return None;
    }
    let mut array = Map::new();
    if truncated {
        array.insert("len".to_owned(), Value::from(after.len()));
    }
    if !at.is_empty() {
        array.insert("at".to_owned(), Value::Object(at));
    }
    if !appended.is_empty() {
        array.insert("push".to_owned(), Value::Array(appended.to_vec()));
    }
    let mut op = Map::new();
    op.insert("arr".to_owned(), Value::Object(array));
    Some(Value::Object(op))
}

/// 把补丁打回去，用来验证「差分 + 应用」确实还原出新视图。
///
/// 这是测试用的参考实现，正式的应用发生在网页端（`viewPatch.ts`），两边
/// 必须对同一套操作码有同样的理解。
#[cfg(test)]
pub(super) fn apply(value: &Value, patch: &Value) -> Value {
    let Some(op) = patch.as_object() else {
        return value.clone();
    };
    if let Some(set) = op.get("set") {
        return set.clone();
    }
    if let Some(array) = op.get("arr").and_then(Value::as_object) {
        let mut items = value.as_array().cloned().unwrap_or_default();
        if let Some(len) = array.get("len").and_then(Value::as_u64) {
            items.truncate(usize::try_from(len).expect("length fits usize"));
        }
        if let Some(at) = array.get("at").and_then(Value::as_object) {
            for (index, child) in at {
                let index: usize = index.parse().expect("index is a number");
                items[index] = apply(&items[index], child);
            }
        }
        if let Some(push) = array.get("push").and_then(Value::as_array) {
            items.extend(push.iter().cloned());
        }
        return Value::Array(items);
    }
    let mut fields = value.as_object().cloned().unwrap_or_default();
    if let Some(removed) = op.get("del").and_then(Value::as_array) {
        for key in removed {
            fields.remove(key.as_str().expect("key is a string"));
        }
    }
    if let Some(changed) = op.get("obj").and_then(Value::as_object) {
        for (key, child) in changed {
            let previous = fields.get(key).cloned().unwrap_or(Value::Null);
            fields.insert(key.clone(), apply(&previous, child));
        }
    }
    Value::Object(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identical_views_produce_no_patch() {
        let view = json!({"version": 3, "players": [{"seat": 0, "points": 25000}]});
        assert!(diff(&view, &view).is_none());
    }

    #[test]
    fn only_the_changed_branch_survives() {
        let before = json!({
            "version": 3,
            "players": [
                {"seat": 0, "nickname": "阿伟", "points": 25000},
                {"seat": 1, "nickname": "小林", "points": 25000},
            ],
        });
        let after = json!({
            "version": 4,
            "players": [
                {"seat": 0, "nickname": "阿伟", "points": 23000},
                {"seat": 1, "nickname": "小林", "points": 25000},
            ],
        });
        let patch = diff(&before, &after).expect("something changed");
        assert_eq!(
            patch,
            json!({"obj": {
                "version": {"set": 4},
                "players": {"arr": {"at": {"0": {"obj": {"points": {"set": 23000}}}}}},
            }})
        );
        // 昵称一次都没出现在补丁里。
        assert!(!patch.to_string().contains("阿伟"));
        assert_eq!(apply(&before, &patch), after);
    }

    #[test]
    fn a_growing_river_only_sends_the_new_tile() {
        let before = json!({"discards": [{"id": 1}, {"id": 2}]});
        let after = json!({"discards": [{"id": 1}, {"id": 2}, {"id": 9}]});
        let patch = diff(&before, &after).expect("a tile was discarded");
        assert_eq!(
            patch,
            json!({"obj": {"discards": {"arr": {"push": [{"id": 9}]}}}})
        );
        assert_eq!(apply(&before, &patch), after);
    }

    #[test]
    fn a_shrinking_array_carries_its_new_length() {
        let before = json!([1, 2, 3, 4]);
        let after = json!([1, 9]);
        let patch = diff(&before, &after).expect("the array shrank");
        assert_eq!(patch, json!({"arr": {"len": 2, "at": {"1": {"set": 9}}}}));
        assert_eq!(apply(&before, &patch), after);
    }

    #[test]
    fn added_and_removed_keys_are_both_described() {
        let before = json!({"exit_vote": null, "gone": 1});
        let after = json!({"exit_vote": {"initiator_seat": 2}, "fresh": true});
        let patch = diff(&before, &after).expect("keys moved");
        assert_eq!(apply(&before, &patch), after);
    }

    #[test]
    fn a_type_change_replaces_the_whole_node() {
        let before = json!({"result": null});
        let after = json!({"result": {"seats": [0, 1]}});
        assert_eq!(
            diff(&before, &after).expect("the result appeared"),
            json!({"obj": {"result": {"set": {"seats": [0, 1]}}}})
        );
    }

    /// 一家的视图片段，带上真实视图里那几个每次都一模一样的身份字段。
    fn player(seat: u64, discards: Vec<Value>) -> Value {
        json!({
            "seat": seat,
            "user_id": format!("user_019fd9d9-03f2-7b90-82df-89814bac7a6{seat}"),
            "nickname": format!("玩家{seat}"),
            "avatar_path": format!("/assets/local-characters/mahjong-soul/ichihime/emotes/{seat}.png"),
            "character_illustration_path":
                "/assets/local-characters/mahjong-soul/ichihime/skins/default.png",
            "points": 25000,
            "melds": [],
            "waiting_tiles": [],
            "furiten": false,
            "discards": discards,
        })
    }

    #[test]
    fn applying_a_patch_reproduces_the_next_view_exactly() {
        let before = json!({
            "version": 41,
            "phase": {"kind": "awaiting_discard", "seat": 1},
            "players": [
                player(0, vec![json!({"id": 1})]),
                player(1, Vec::new()),
                player(2, Vec::new()),
                player(3, Vec::new()),
            ],
            "clocks": [{"seat": 0, "remaining_ms": 9000}],
        });
        let mut after = before.clone();
        after["version"] = json!(42);
        after["phase"] = json!({"kind": "awaiting_reaction", "seat": 2});
        after["players"][1]["discards"] = json!([{"id": 7}]);
        after["players"][1]["furiten"] = json!(true);
        after["clocks"][0]["remaining_ms"] = json!(4000);

        let patch = diff(&before, &after).expect("the hand advanced");
        assert_eq!(apply(&before, &patch), after);
        assert!(
            patch.to_string().len() * 5 < after.to_string().len(),
            "一次推进的补丁必须远小于整份视图，否则这一层白做了：{} vs {}",
            patch.to_string().len(),
            after.to_string().len(),
        );
    }
}
