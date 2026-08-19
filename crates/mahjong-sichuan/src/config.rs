//! 四川麻将的规则配置。
//!
//! 形状和 `mahjong-impact` 的 `config.rs` 一致：全部 `deny_unknown_fields`，
//! 房间请求走「预设 + 覆盖」解析成一份完整配置再校验（见 `overrides.rs`）。
//! 四川麻将血战到底只有「思考秒数」一项可调，其余规则（番型、杠、流局、局数）写死。

use serde::{Deserialize, Serialize};

/// 每人的起始点数：血战到底从 0 分打起。
pub const INITIAL_POINTS: i32 = 0;

/// 座位数：四川麻将固定四人。
pub const SEAT_COUNT: u8 = 4;

/// 局数：血战到底固定打 4 局。
pub const HAND_COUNT: u8 = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThinkingTimeRules {
    pub base_seconds: u16,
    pub reserve_seconds: u16,
}

impl ThinkingTimeRules {
    #[must_use]
    pub const fn base_ms(self) -> u64 {
        self.base_seconds as u64 * 1_000
    }

    #[must_use]
    pub const fn reserve_ms(self) -> u32 {
        self.reserve_seconds as u32 * 1_000
    }
}

impl Default for ThinkingTimeRules {
    fn default() -> Self {
        Self {
            base_seconds: 5,
            reserve_seconds: 20,
        }
    }
}

/// 对局设置。四川麻将只有思考秒数一项。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRules {
    #[serde(default)]
    pub thinking_time: ThinkingTimeRules,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SichuanRules {
    #[serde(default)]
    pub match_rules: MatchRules,
}

impl SichuanRules {
    /// 标准配置：思考时间 5 + 20 秒。
    #[must_use]
    pub fn standard() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::SichuanRules;

    #[test]
    fn standard_matches_the_documented_defaults() {
        let rules = SichuanRules::standard();

        assert_eq!(rules.match_rules.thinking_time.base_seconds, 5);
        assert_eq!(rules.match_rules.thinking_time.reserve_seconds, 20);
    }

    #[test]
    fn rejects_unknown_config_fields() {
        let error = serde_json::from_str::<SichuanRules>(r#"{"unknown": true}"#);

        assert!(error.is_err());
    }

    #[test]
    fn config_round_trips_through_json() {
        let rules = SichuanRules::standard();
        let json = serde_json::to_string(&rules).expect("serializes");
        let parsed: SichuanRules = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(parsed, rules);
    }
}
