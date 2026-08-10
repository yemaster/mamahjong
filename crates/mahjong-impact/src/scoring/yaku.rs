//! 番种与全交种类。
//!
//! 冲击麻将的点数是「底和 12 + 各番种点数之和」，没有符、没有翻倍。

/// 番种。点数见 [`Yaku::value`]。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Yaku {
    /// 无财神。
    NoJoker,
    /// 两财神。
    TwoJokers,
    /// 三财神。
    ThreeJokers,
    /// 七对子。
    SevenPairs,
    /// 七嵌。
    SevenGaps,
    /// 对对和。
    AllTriplets,
    /// 十三不搭。
    ThirteenUnrelated,
    /// 七风齐。
    SevenWinds,
    /// 清一色（财神当了别的牌）。
    PureFlush,
    /// 抛龙。
    PaoLong,
    /// 杠上开花。
    RinshanKaihou,
    /// 单吊。
    SingleWait,
    /// 连庄，每次 +10。
    DealerStreak,
    /// 全风（对应全交项关闭时）。
    AllHonors,
    /// 无龙清一色（对应全交项关闭时）。
    PureFlushNoJoker,
    /// 清七对（对应全交项关闭时）。
    PureSevenPairs,
    /// 海底（对应全交项关闭时）。
    LastTile,
    /// 天和地和（对应全交项关闭时）。
    Blessing,
    /// 三杠（对应全交项关闭时）。
    ThreeKans,
    /// 四龙（对应全交项关闭时）。
    FourJokers,
    /// 连打十一风（对应全交项关闭时）。
    ElevenHonorStreak,
}

impl Yaku {
    /// 单次计入时的点数。连庄要乘以连庄次数，见 [`YakuValue`]。
    #[must_use]
    pub const fn value(self) -> u32 {
        match self {
            Self::NoJoker
            | Self::TwoJokers
            | Self::SevenPairs
            | Self::SevenGaps
            | Self::AllTriplets
            | Self::ThirteenUnrelated
            | Self::SevenWinds => 1,
            Self::ThreeJokers => 2,
            Self::PureFlush
            | Self::PaoLong
            | Self::RinshanKaihou
            | Self::SingleWait
            | Self::DealerStreak
            | Self::AllHonors
            | Self::PureFlushNoJoker
            | Self::PureSevenPairs
            | Self::LastTile
            | Self::Blessing
            | Self::ThreeKans
            | Self::FourJokers
            | Self::ElevenHonorStreak => 10,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoJoker => "no_joker",
            Self::TwoJokers => "two_jokers",
            Self::ThreeJokers => "three_jokers",
            Self::SevenPairs => "seven_pairs",
            Self::SevenGaps => "seven_gaps",
            Self::AllTriplets => "all_triplets",
            Self::ThirteenUnrelated => "thirteen_unrelated",
            Self::SevenWinds => "seven_winds",
            Self::PureFlush => "pure_flush",
            Self::PaoLong => "pao_long",
            Self::RinshanKaihou => "rinshan_kaihou",
            Self::SingleWait => "single_wait",
            Self::DealerStreak => "dealer_streak",
            Self::AllHonors => "all_honors",
            Self::PureFlushNoJoker => "pure_flush_no_joker",
            Self::PureSevenPairs => "pure_seven_pairs",
            Self::LastTile => "last_tile",
            Self::Blessing => "blessing",
            Self::ThreeKans => "three_kans",
            Self::FourJokers => "four_jokers",
            Self::ElevenHonorStreak => "eleven_honor_streak",
        }
    }
}

/// 一条已计入的番种：番种本身、重复次数、合计点数。
///
/// 只有连庄会出现 `count > 1`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct YakuValue {
    yaku: Yaku,
    count: u32,
}

impl YakuValue {
    #[must_use]
    pub const fn new(yaku: Yaku, count: u32) -> Self {
        Self { yaku, count }
    }

    #[must_use]
    pub const fn single(yaku: Yaku) -> Self {
        Self::new(yaku, 1)
    }

    #[must_use]
    pub const fn yaku(self) -> Yaku {
        self.yaku
    }

    #[must_use]
    pub const fn count(self) -> u32 {
        self.count
    }

    #[must_use]
    pub const fn points(self) -> u32 {
        self.yaku.value() * self.count
    }
}

/// 全交种类。触发之后胜者点数变成 400，其余三家归零。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllInKind {
    /// 连打十一风。
    ElevenHonorStreak,
    /// 四龙。
    FourJokers,
    /// 三杠。
    ThreeKans,
    /// 全风。
    AllHonors,
    /// 无龙清一色。
    PureFlushNoJoker,
    /// 清七对。
    PureSevenPairs,
    /// 单吊。
    SingleWait,
    /// 海底。
    LastTile,
    /// 天和地和。
    Blessing,
}

impl AllInKind {
    /// 同时成立时的取用顺序：先手性最强的排前面。
    pub const PRIORITY: [Self; 9] = [
        Self::ElevenHonorStreak,
        Self::FourJokers,
        Self::ThreeKans,
        Self::AllHonors,
        Self::PureFlushNoJoker,
        Self::PureSevenPairs,
        Self::SingleWait,
        Self::LastTile,
        Self::Blessing,
    ];

    /// 该项关闭时降级成的番种（额外 +10 点）。
    #[must_use]
    pub const fn fallback_yaku(self) -> Yaku {
        match self {
            Self::ElevenHonorStreak => Yaku::ElevenHonorStreak,
            Self::FourJokers => Yaku::FourJokers,
            Self::ThreeKans => Yaku::ThreeKans,
            Self::AllHonors => Yaku::AllHonors,
            Self::PureFlushNoJoker => Yaku::PureFlushNoJoker,
            Self::PureSevenPairs => Yaku::PureSevenPairs,
            Self::SingleWait => Yaku::SingleWait,
            Self::LastTile => Yaku::LastTile,
            Self::Blessing => Yaku::Blessing,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ElevenHonorStreak => "eleven_honor_streak",
            Self::FourJokers => "four_jokers",
            Self::ThreeKans => "three_kans",
            Self::AllHonors => "all_honors",
            Self::PureFlushNoJoker => "pure_flush_no_joker",
            Self::PureSevenPairs => "pure_seven_pairs",
            Self::SingleWait => "single_wait",
            Self::LastTile => "last_tile",
            Self::Blessing => "blessing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AllInKind, Yaku, YakuValue};

    #[test]
    fn base_table_values_match_the_rule_book() {
        assert_eq!(Yaku::NoJoker.value(), 1);
        assert_eq!(Yaku::TwoJokers.value(), 1);
        assert_eq!(Yaku::ThreeJokers.value(), 2);
        assert_eq!(Yaku::SevenPairs.value(), 1);
        assert_eq!(Yaku::SevenGaps.value(), 1);
        assert_eq!(Yaku::AllTriplets.value(), 1);
        assert_eq!(Yaku::ThirteenUnrelated.value(), 1);
        assert_eq!(Yaku::SevenWinds.value(), 1);
        assert_eq!(Yaku::PureFlush.value(), 10);
        assert_eq!(Yaku::PaoLong.value(), 10);
        assert_eq!(Yaku::RinshanKaihou.value(), 10);
        assert_eq!(Yaku::SingleWait.value(), 10);
    }

    #[test]
    fn dealer_streak_multiplies_by_the_streak_count() {
        assert_eq!(YakuValue::new(Yaku::DealerStreak, 3).points(), 30);
        assert_eq!(YakuValue::single(Yaku::PureFlush).points(), 10);
    }

    #[test]
    fn every_all_in_kind_has_a_ten_point_fallback() {
        for kind in AllInKind::PRIORITY {
            assert_eq!(kind.fallback_yaku().value(), 10);
        }
    }

    #[test]
    fn all_in_names_and_fallback_names_line_up() {
        for kind in AllInKind::PRIORITY {
            assert_eq!(kind.as_str(), kind.fallback_yaku().as_str());
        }
    }
}
