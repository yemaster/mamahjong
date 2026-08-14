//! 冲击麻将的规则配置。
//!
//! 形状和 `mahjong-riichi` 的 `config.rs` 一致：全部 `deny_unknown_fields`，
//! 房间请求走「预设 + 覆盖」解析成一份完整配置再校验（见 `overrides.rs`）。

use serde::{Deserialize, Serialize};

/// 每人的起始点数。
pub const INITIAL_POINTS: i32 = 100;

/// 每人的起始杠点。
pub const INITIAL_KAN_POINTS: i32 = 0;

/// 全交时胜者拿到的点数，其余三家归零。
pub const ALL_IN_WINNER_POINTS: i32 = 400;

/// 底和。
pub const BASE_VALUE: u32 = 12;

/// 每次连庄给和牌者加的点数。
pub const DEALER_STREAK_VALUE: u32 = 10;

/// 座位数：冲击麻将固定四人。
pub const SEAT_COUNT: u8 = 4;

/// 模式。「瞎子麻将」保持只自摸、不能吃；「亮子麻将」开放吃、荣和与抢杠。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactMode {
    #[default]
    Blind,
    Bright,
}

impl ImpactMode {
    #[must_use]
    pub const fn allows_open_wins(self) -> bool {
        matches!(self, Self::Bright)
    }
}

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

/// 对局设置。冲击麻将只有思考秒数一项——长度由「有人点数归零」决定，
/// 起始点数、返点、飞、连庄条件都是规则写死的。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRules {
    #[serde(default)]
    pub thinking_time: ThinkingTimeRules,
}

/// 杠牌设置，三项默认全开。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KanRules {
    /// 加杠时仅单人支付：开启则只有被碰那家付 3 杠点，关闭则其余三家各付 1。
    pub added_kan_single_payer: bool,
    /// 指示牌碰牌算杠：碰财神指示牌按明杠结算杠点，手持三张指示牌可按暗杠结算。
    /// 牌型仍然是刻子——不摸岭上牌、不算杠上开花、不计入三杠。
    pub indicator_pon_counts_as_kan: bool,
    /// 第一巡连打需要庄家支付杠点：庄家首打之后无人鸣牌、三家依次打出同一种牌，
    /// 庄家向其余三家各付 1 杠点。
    pub first_round_repeat_discard: bool,
    /// 打出四张相同牌算杠：一个人在牌河中打出四张相同的牌，向另外三人各收 1 杠点；
    /// 连续打出四张（中间无其他牌）则各收 2 杠点。指示牌碰算杠开启时，打三张指示牌
    /// 同样触发（连打三张收双倍）。
    pub four_identical_discards_as_kan: bool,
    /// 手牌 ≦4 张时碰牌收杠点：明碰向打出者收 3 杠点，明杠改为收 6 杠点。
    pub pon_with_few_tiles_as_kan: bool,
}

impl Default for KanRules {
    fn default() -> Self {
        Self {
            added_kan_single_payer: true,
            indicator_pon_counts_as_kan: true,
            first_round_repeat_discard: true,
            four_identical_discards_as_kan: true,
            pon_with_few_tiles_as_kan: true,
        }
    }
}

/// 特殊规则设置。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialRules {
    /// 七嵌：手牌可分为 7 组，每组是同花色相差恰好 2 的两张数牌。默认关闭。
    #[serde(default)]
    pub seven_gaps: bool,
}

/// 全交设置，九项默认全开。
///
/// 开启：胡出该牌型直接全交（胜者 400、其余三家 0）。
/// 关闭：胡出该牌型改为额外 +10 点。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllInRules {
    /// 连打十一风。
    pub eleven_honor_streak: bool,
    /// 全风。
    pub all_honors: bool,
    /// 无龙清一色。
    pub pure_flush_no_joker: bool,
    /// 单吊。
    pub single_wait: bool,
    /// 三杠。
    pub three_kans: bool,
    /// 四龙。
    pub four_jokers: bool,
    /// 清七对。
    pub pure_seven_pairs: bool,
    /// 海底。
    pub last_tile: bool,
    /// 天和地和。
    pub blessing: bool,
}

impl Default for AllInRules {
    fn default() -> Self {
        Self {
            eleven_honor_streak: true,
            all_honors: true,
            pure_flush_no_joker: true,
            single_wait: true,
            three_kans: true,
            four_jokers: true,
            pure_seven_pairs: true,
            last_tile: true,
            blessing: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactRules {
    #[serde(default)]
    pub mode: ImpactMode,
    #[serde(default)]
    pub match_rules: MatchRules,
    #[serde(default)]
    pub kan: KanRules,
    #[serde(default)]
    pub special: SpecialRules,
    #[serde(default)]
    pub all_in: AllInRules,
}

impl ImpactRules {
    /// 标准配置：模式为瞎子麻将，杠牌三项全开，七嵌关闭，全交九项全开。
    #[must_use]
    pub fn standard() -> Self {
        Self::default()
    }

    /// 亮子麻将的建房默认：杠牌附加项全关，只保留连打十一风、四龙和天和地和全交。
    #[must_use]
    pub fn bright() -> Self {
        let mut rules = Self::default();
        rules.mode = ImpactMode::Bright;
        rules.kan = KanRules {
            added_kan_single_payer: false,
            indicator_pon_counts_as_kan: false,
            first_round_repeat_discard: false,
            four_identical_discards_as_kan: false,
            pon_with_few_tiles_as_kan: false,
        };
        rules.all_in = AllInRules {
            eleven_honor_streak: true,
            all_honors: false,
            pure_flush_no_joker: false,
            single_wait: false,
            three_kans: false,
            four_jokers: true,
            pure_seven_pairs: false,
            last_tile: false,
            blessing: true,
        };
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::{AllInRules, ImpactMode, ImpactRules, KanRules};

    #[test]
    fn standard_matches_the_documented_defaults() {
        let rules = ImpactRules::standard();

        assert_eq!(rules.mode, ImpactMode::Blind);
        assert_eq!(rules.match_rules.thinking_time.base_seconds, 5);
        assert_eq!(rules.match_rules.thinking_time.reserve_seconds, 20);
        assert_eq!(
            rules.kan,
            KanRules {
                added_kan_single_payer: true,
                indicator_pon_counts_as_kan: true,
                first_round_repeat_discard: true,
                four_identical_discards_as_kan: true,
                pon_with_few_tiles_as_kan: true,
            }
        );
        assert!(!rules.special.seven_gaps);
        assert_eq!(
            rules.all_in,
            AllInRules {
                eleven_honor_streak: true,
                all_honors: true,
                pure_flush_no_joker: true,
                single_wait: true,
                three_kans: true,
                four_jokers: true,
                pure_seven_pairs: true,
                last_tile: true,
                blessing: true,
            }
        );
    }

    #[test]
    fn bright_matches_the_room_picker_defaults() {
        let rules = ImpactRules::bright();

        assert_eq!(rules.mode, ImpactMode::Bright);
        assert!(!rules.kan.added_kan_single_payer);
        assert!(!rules.kan.indicator_pon_counts_as_kan);
        assert!(!rules.kan.first_round_repeat_discard);
        assert!(!rules.kan.four_identical_discards_as_kan);
        assert!(!rules.kan.pon_with_few_tiles_as_kan);
        assert!(rules.all_in.eleven_honor_streak);
        assert!(rules.all_in.four_jokers);
        assert!(rules.all_in.blessing);
        assert!(!rules.all_in.all_honors);
        assert!(!rules.all_in.pure_flush_no_joker);
        assert!(!rules.all_in.single_wait);
        assert!(!rules.all_in.three_kans);
        assert!(!rules.all_in.pure_seven_pairs);
        assert!(!rules.all_in.last_tile);
    }

    #[test]
    fn rejects_unknown_config_fields() {
        let error = serde_json::from_str::<ImpactRules>(r#"{"unknown": true}"#);

        assert!(error.is_err());
    }

    #[test]
    fn config_round_trips_through_json() {
        let rules = ImpactRules::standard();
        let json = serde_json::to_string(&rules).expect("serializes");
        let parsed: ImpactRules = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(parsed, rules);
    }
}
