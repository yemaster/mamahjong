//! 规则类型往外发时用的名字。
//!
//! 同一批名字有两个出口：实时对局的 HTTP/WS 响应（`apps/server/src/api/dto.rs`）
//! 和归档下来的牌谱（`record.rs`）。两边写的必须是同一套字符串——牌谱重演直接把
//! 结算面板那套组件拿去用，名字对不上就是两份数据画出两种界面。所以这张表只留一份。
//!
//! 场风和终局原因写英文 key（前端 `types.ts` 的 union 直接对着解），役种和番缚写
//! 中文（前端原样上屏，不再翻一遍）。

use mahjong_impact::{
    AllInKind, EndReason as ImpactEndReason, ImpactPreset, ImpactRuleSnapshot, ImpactRules,
    Yaku as ImpactYaku,
};
use mahjong_riichi::{EndReason, Limit, RiichiPreset, RiichiRuleSnapshot, RiichiRules, Wind, Yaku};

/// 场风：前端 `Wind` union 的取值。
#[must_use]
pub const fn wind_name(value: Wind) -> &'static str {
    match value {
        Wind::East => "east",
        Wind::South => "south",
        Wind::West => "west",
        Wind::North => "north",
    }
}

/// 一局是怎么结束的：前端 `EndReason` union 的取值。
#[must_use]
pub const fn end_reason_name(value: EndReason) -> &'static str {
    match value {
        EndReason::ExhaustiveDraw => "exhaustive_draw",
        EndReason::NineTerminals => "nine_terminals",
        EndReason::FourWinds => "four_winds",
        EndReason::FourKans => "four_kans",
        EndReason::FourRiichi => "four_riichi",
        EndReason::Tsumo => "tsumo",
        EndReason::Ron => "ron",
    }
}

/// 番缚的中文名，没到满贯写空串。
#[must_use]
pub const fn limit_name(value: Limit) -> &'static str {
    match value {
        Limit::None => "",
        Limit::Mangan => "满贯",
        Limit::Haneman => "跳满",
        Limit::Baiman => "倍满",
        Limit::Sanbaiman => "三倍满",
        Limit::KazoeYakuman => "累计役满",
        Limit::Yakuman(1) => "役满",
        Limit::Yakuman(2) => "双倍役满",
        Limit::Yakuman(3) => "三倍役满",
        Limit::Yakuman(_) => "多倍役满",
    }
}

/// 役种的中文名。
#[must_use]
pub const fn yaku_name(value: Yaku) -> &'static str {
    match value {
        Yaku::MenzenTsumo => "门前清自摸和",
        Yaku::Riichi => "立直",
        Yaku::DoubleRiichi => "两立直",
        Yaku::Ippatsu => "一发",
        Yaku::SeatWind => "自风",
        Yaku::RoundWind => "场风",
        Yaku::WhiteDragon => "役牌白",
        Yaku::GreenDragon => "役牌发",
        Yaku::RedDragon => "役牌中",
        Yaku::North => "役牌北",
        Yaku::Pinfu => "平和",
        Yaku::Tanyao => "断幺九",
        Yaku::Iipeikou => "一杯口",
        Yaku::Haitei => "海底摸月",
        Yaku::Houtei => "河底捞鱼",
        Yaku::Chankan => "抢杠",
        Yaku::Rinshan => "岭上开花",
        Yaku::SevenPairs => "七对子",
        Yaku::Toitoi => "对对和",
        Yaku::Sanankou => "三暗刻",
        Yaku::SanshokuDoukou => "三色同刻",
        Yaku::Sankantsu => "三杠子",
        Yaku::Shousangen => "小三元",
        Yaku::Honroutou => "混老头",
        Yaku::SanshokuDoujun => "三色同顺",
        Yaku::Ittsu => "一气通贯",
        Yaku::Chanta => "混全带幺九",
        Yaku::Ryanpeikou => "二杯口",
        Yaku::Honitsu => "混一色",
        Yaku::Junchan => "纯全带幺九",
        Yaku::Chinitsu => "清一色",
        Yaku::Tenhou => "天和",
        Yaku::Chiihou => "地和",
        Yaku::ThirteenOrphans => "国士无双",
        Yaku::ThirteenWaitOrphans => "国士无双十三面",
        Yaku::Suuankou => "四暗刻",
        Yaku::SuuankouTanki => "四暗刻单骑",
        Yaku::Daisangen => "大三元",
        Yaku::Ryuuiisou => "绿一色",
        Yaku::Tsuuiisou => "字一色",
        Yaku::Shousuushi => "小四喜",
        Yaku::Daisuushi => "大四喜",
        Yaku::Chinroutou => "清老头",
        Yaku::Suukantsu => "四杠子",
        Yaku::ChuurenPoutou => "九莲宝灯",
        Yaku::PureChuurenPoutou => "纯正九莲宝灯",
        Yaku::Renhou => "人和",
        Yaku::Daisharin => "大车轮",
        Yaku::Daichikurin => "大竹林",
        Yaku::Daisuurin => "大数邻",
        _ => "役种",
    }
}

/// 一份规则快照该写成什么名字。
///
/// 牌谱标题上「四人南」后面还要跟一句规则名。三种情况：
///
/// - 挂着预设、且配置和那份预设一字不差 → 预设的短名（「ML规则」「A规」）。
/// - 挂着预设、但配置被房主动过 → 「自定义规则」。快照里的预设引用在改过之后也
///   还留着（`ResolvedRiichiRules` 不会因为一次覆盖就把出处抹掉），所以只看有没有
///   预设引用判断不出改没改过，得拿配置本身跟预设比。
/// - 没挂预设 → 和该人数的标准规则一致就写「标准规则」，否则同样是「自定义规则」。
#[must_use]
pub fn rule_display_name(snapshot: &RiichiRuleSnapshot) -> &'static str {
    let rules = snapshot.rules();
    match snapshot
        .preset()
        .and_then(|preset| RiichiPreset::find(preset.id().as_str()))
    {
        Some(preset) if preset.rules() == *rules => preset.short_name(),
        Some(_) => "自定义规则",
        None if RiichiRules::standard(rules.variant) == *rules => "标准规则",
        None => "自定义规则",
    }
}

/// 冲击麻将役种的中文名。
///
/// 和立直那张表分开写：两套规则的番种毫无交集，混在一起只会让人以为它们有关系。
#[must_use]
pub const fn impact_yaku_name(value: ImpactYaku) -> &'static str {
    match value {
        ImpactYaku::NoJoker => "无财神",
        ImpactYaku::TwoJokers => "两财神",
        ImpactYaku::ThreeJokers => "三财神",
        ImpactYaku::SevenPairs => "七对子",
        ImpactYaku::SevenGaps => "七嵌",
        ImpactYaku::AllTriplets => "对对和",
        ImpactYaku::ThirteenUnrelated => "十三不搭",
        ImpactYaku::SevenWinds => "七风齐",
        ImpactYaku::PureFlush => "清一色",
        ImpactYaku::PaoLong => "抛龙",
        ImpactYaku::RinshanKaihou => "杠上开花",
        ImpactYaku::Chankan => "抢杠",
        ImpactYaku::SingleWait => "单吊",
        ImpactYaku::DealerStreak => "连庄",
        ImpactYaku::AllHonors => "全风",
        ImpactYaku::PureFlushNoJoker => "无龙清一色",
        ImpactYaku::PureSevenPairs => "清七对",
        ImpactYaku::LastTile => "海底",
        ImpactYaku::Blessing => "天和地和",
        ImpactYaku::ThreeKans => "三杠",
        ImpactYaku::FourJokers => "四龙",
        ImpactYaku::ElevenHonorStreak => "连打十一风",
    }
}

/// 全交种类的中文名，和建房面板里那九个开关一字不差。
#[must_use]
pub const fn impact_all_in_name(value: AllInKind) -> &'static str {
    match value {
        AllInKind::ElevenHonorStreak => "连打十一风全交",
        AllInKind::FourJokers => "四龙全交",
        AllInKind::ThreeKans => "三杠全交",
        AllInKind::AllHonors => "全风全交",
        AllInKind::PureFlushNoJoker => "无龙清一色全交",
        AllInKind::PureSevenPairs => "清七对全交",
        AllInKind::SingleWait => "单吊全交",
        AllInKind::LastTile => "海底全交",
        AllInKind::Blessing => "天和地和全交",
    }
}

/// 冲击麻将一局是怎么结束的：前端 union 的取值。
#[must_use]
pub const fn impact_end_reason_name(value: ImpactEndReason) -> &'static str {
    value.as_str()
}

/// 冲击麻将的规则名。瞎子与亮子的建房默认都叫标准规则，手调后才叫自定义。
#[must_use]
pub fn impact_rule_display_name(snapshot: &ImpactRuleSnapshot) -> &'static str {
    let rules = snapshot.rules();
    match snapshot
        .preset()
        .and_then(|preset| ImpactPreset::find(preset.id().as_str()))
    {
        Some(preset) if preset.rules() == *rules => preset.short_name(),
        Some(_) => "自定义规则",
        None if ImpactRules::standard() == *rules || ImpactRules::bright() == *rules => "标准规则",
        None => "自定义规则",
    }
}
