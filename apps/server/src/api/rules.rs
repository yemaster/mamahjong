use axum::Json;
use axum::Router;
use axum::routing::get;
use mahjong_impact::ImpactRules;
use mahjong_riichi::{RiichiPreset, RiichiRules, RiichiVariant};
use serde::Serialize;

use crate::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/rule-sets", get(rule_sets))
}

#[derive(Serialize)]
struct RuleSetCatalogResponse {
    schema: &'static str,
    rule_sets: Vec<RuleSetResponse>,
}

/// 目录里的一套规则。
///
/// `default_config` 与 `presets[].config` 的形状取决于 `family`：`riichi` 是
/// `RiichiRules`，`impact` 是 `ImpactRules`。两者字段完全不同，所以这里统一收成
/// `serde_json::Value`，由前端照 `family` 分支解读。
#[derive(Serialize)]
struct RuleSetResponse {
    id: &'static str,
    family: &'static str,
    display_name: &'static str,
    seat_count: u8,
    default_config: serde_json::Value,
    presets: Vec<PresetResponse>,
}

#[derive(Serialize)]
struct PresetResponse {
    id: &'static str,
    revision: u32,
    display_name: &'static str,
    config: serde_json::Value,
}

/// 目录是常量拼出来的，序列化不会失败；真失败了也只该少一套规则，不该 500。
fn config_of<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

async fn rule_sets() -> Json<RuleSetCatalogResponse> {
    Json(RuleSetCatalogResponse {
        schema: "rule_set_catalog.v1",
        rule_sets: vec![
            RuleSetResponse {
                id: "riichi/yonma",
                family: "riichi",
                display_name: "四人日麻",
                seat_count: 4,
                default_config: config_of(&RiichiRules::standard(RiichiVariant::Yonma)),
                presets: RiichiPreset::ALL
                    .into_iter()
                    .map(|preset| PresetResponse {
                        id: preset.id(),
                        revision: preset.revision().get(),
                        display_name: preset.display_name(),
                        config: config_of(&preset.rules()),
                    })
                    .collect(),
            },
            RuleSetResponse {
                id: "riichi/sanma",
                family: "riichi",
                display_name: "三人日麻",
                seat_count: 3,
                default_config: config_of(&RiichiRules::standard(RiichiVariant::Sanma)),
                presets: Vec::new(),
            },
            RuleSetResponse {
                id: "impact/yonma",
                family: "impact",
                display_name: "冲击麻将",
                seat_count: 4,
                default_config: config_of(&ImpactRules::standard()),
                /*
                 * 冲击麻将没有流派之分，`ImpactPreset::Standard` 就是
                 * `ImpactRules::standard()` 本身。把它摆进目录，建房页上就会
                 * 多出一个「标准 / 标准 / 自定义」的下拉框——三个选项里有两个
                 * 是同一套规则。解析路径上那一层留着（将来真有流派时不用改
                 * 调用方），但目录里不对外宣告。
                 */
                presets: Vec::new(),
            },
        ],
    })
}
