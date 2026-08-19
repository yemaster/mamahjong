//! 四川麻将的番型。
//!
//! 分数 = 1000 × 2^(番−1)，封顶 6 番（32000 分）。基础番型取最高、不叠加；加番叠加。

/// 番型。番值见 [`Yaku::fan`]。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Yaku {
    /// 平胡（基础 1）。
    PingHu,
    /// 对对胡（基础 2）。
    DuiDuiHu,
    /// 清一色（基础 3）。
    QingYiSe,
    /// 七对（基础 3）。
    QiDui,
    /// 清对：清一色 + 对对胡（基础 4）。
    QingDui,
    /// 龙七对：七对含四张相同（基础 5）。
    LongQiDui,
    /// 清七对：清一色 + 七对（基础 5）。
    QingQiDui,
    /// 天胡 / 地胡（基础 6）。
    TianHuDiHu,
    /// 自摸（加番 1）。
    ZiMo,
    /// 根：每个杠（加番 1）。
    Gen,
    /// 杠上花（加番 1）。
    GangShangHua,
    /// 杠上炮（加番 1）。
    GangShangPao,
    /// 抢杠胡（加番 1）。
    QiangGangHu,
    /// 金钩钓：全副露单钓将（加番 1）。
    JinGouDiao,
    /// 海底（加番 1）。
    HaiDi,
}

impl Yaku {
    /// 单次计入时的番数。
    #[must_use]
    pub const fn fan(self) -> u32 {
        match self {
            Self::PingHu => 1,
            Self::DuiDuiHu => 2,
            Self::QingYiSe | Self::QiDui => 3,
            Self::QingDui => 4,
            Self::LongQiDui | Self::QingQiDui => 5,
            Self::TianHuDiHu => 6,
            Self::ZiMo
            | Self::Gen
            | Self::GangShangHua
            | Self::GangShangPao
            | Self::QiangGangHu
            | Self::JinGouDiao
            | Self::HaiDi => 1,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PingHu => "ping_hu",
            Self::DuiDuiHu => "dui_dui_hu",
            Self::QingYiSe => "qing_yi_se",
            Self::QiDui => "qi_dui",
            Self::QingDui => "qing_dui",
            Self::LongQiDui => "long_qi_dui",
            Self::QingQiDui => "qing_qi_dui",
            Self::TianHuDiHu => "tian_hu_di_hu",
            Self::ZiMo => "zi_mo",
            Self::Gen => "gen",
            Self::GangShangHua => "gang_shang_hua",
            Self::GangShangPao => "gang_shang_pao",
            Self::QiangGangHu => "qiang_gang_hu",
            Self::JinGouDiao => "jin_gou_diao",
            Self::HaiDi => "hai_di",
        }
    }
}

/// 一条已计入的番种：番种本身、重复次数（只有「根」会出现 `count > 1`）。
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
    pub const fn fan(self) -> u32 {
        self.yaku.fan() * self.count
    }
}

#[cfg(test)]
mod tests {
    use super::{Yaku, YakuValue};

    #[test]
    fn fan_table_matches_the_rule_book() {
        assert_eq!(Yaku::PingHu.fan(), 1);
        assert_eq!(Yaku::DuiDuiHu.fan(), 2);
        assert_eq!(Yaku::QingYiSe.fan(), 3);
        assert_eq!(Yaku::QiDui.fan(), 3);
        assert_eq!(Yaku::QingDui.fan(), 4);
        assert_eq!(Yaku::LongQiDui.fan(), 5);
        assert_eq!(Yaku::QingQiDui.fan(), 5);
        assert_eq!(Yaku::TianHuDiHu.fan(), 6);
        assert_eq!(Yaku::ZiMo.fan(), 1);
        assert_eq!(Yaku::Gen.fan(), 1);
    }

    #[test]
    fn gen_multiplies_by_the_kan_count() {
        assert_eq!(YakuValue::new(Yaku::Gen, 3).fan(), 3);
        assert_eq!(YakuValue::single(Yaku::PingHu).fan(), 1);
    }
}
