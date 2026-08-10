#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum Yaku {
    MenzenTsumo,
    Riichi,
    DoubleRiichi,
    Ippatsu,
    SeatWind,
    RoundWind,
    WhiteDragon,
    GreenDragon,
    RedDragon,
    Pinfu,
    Tanyao,
    Iipeikou,
    Haitei,
    Houtei,
    Chankan,
    Rinshan,
    SevenPairs,
    Toitoi,
    Sanankou,
    SanshokuDoukou,
    Sankantsu,
    Shousangen,
    Honroutou,
    SanshokuDoujun,
    Ittsu,
    Chanta,
    Ryanpeikou,
    Honitsu,
    Junchan,
    Chinitsu,
    Tenhou,
    Chiihou,
    ThirteenOrphans,
    ThirteenWaitOrphans,
    Suuankou,
    SuuankouTanki,
    Daisangen,
    Ryuuiisou,
    Tsuuiisou,
    Shousuushi,
    Daisuushi,
    Chinroutou,
    Suukantsu,
    ChuurenPoutou,
    PureChuurenPoutou,
    Renhou,
    Daisharin,
    Daichikurin,
    Daisuurin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct YakuValue {
    yaku: Yaku,
    value: u8,
    yakuman: bool,
}

impl YakuValue {
    pub(super) const fn han(yaku: Yaku, value: u8) -> Self {
        Self {
            yaku,
            value,
            yakuman: false,
        }
    }

    pub(super) const fn yakuman(yaku: Yaku, multiplier: u8) -> Self {
        Self {
            yaku,
            value: multiplier,
            yakuman: true,
        }
    }

    #[must_use]
    pub const fn yaku(self) -> Yaku {
        self.yaku
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.value
    }

    #[must_use]
    pub const fn is_yakuman(self) -> bool {
        self.yakuman
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BonusHan {
    dora: u8,
    ura_dora: u8,
    red_dora: u8,
}

impl BonusHan {
    pub(super) const fn new(dora: u8, ura_dora: u8, red_dora: u8) -> Self {
        Self {
            dora,
            ura_dora,
            red_dora,
        }
    }

    #[must_use]
    pub const fn dora(self) -> u8 {
        self.dora
    }

    #[must_use]
    pub const fn ura_dora(self) -> u8 {
        self.ura_dora
    }

    #[must_use]
    pub const fn red_dora(self) -> u8 {
        self.red_dora
    }

    #[must_use]
    pub const fn total(self) -> u8 {
        self.dora + self.ura_dora + self.red_dora
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HandShape {
    Standard,
    SevenPairs,
    ThirteenOrphans,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WaitKind {
    TwoSided,
    Edge,
    Closed,
    Pair,
    DoublePair,
    ThirteenSided,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Limit {
    None,
    Mangan,
    Haneman,
    Baiman,
    Sanbaiman,
    KazoeYakuman,
    Yakuman(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Payment {
    Ron {
        points: u32,
    },
    Tsumo {
        dealer_payment: u32,
        other_payment: u32,
    },
}

impl Payment {
    #[must_use]
    pub const fn total_received(self, player_count: u8, winner_is_dealer: bool) -> u32 {
        match self {
            Self::Ron { points } => points,
            Self::Tsumo {
                dealer_payment,
                other_payment,
            } => {
                if winner_is_dealer {
                    other_payment * (player_count as u32 - 1)
                } else {
                    dealer_payment + other_payment * (player_count as u32 - 2)
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinEvaluation {
    shape: HandShape,
    wait: WaitKind,
    yaku: Box<[YakuValue]>,
    bonuses: BonusHan,
    han: u8,
    fu: u16,
    yakuman_multiplier: u8,
    base_points: u32,
    limit: Limit,
    payment: Payment,
}

impl WinEvaluation {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        shape: HandShape,
        wait: WaitKind,
        yaku: impl Into<Box<[YakuValue]>>,
        bonuses: BonusHan,
        han: u8,
        fu: u16,
        yakuman_multiplier: u8,
        base_points: u32,
        limit: Limit,
        payment: Payment,
    ) -> Self {
        Self {
            shape,
            wait,
            yaku: yaku.into(),
            bonuses,
            han,
            fu,
            yakuman_multiplier,
            base_points,
            limit,
            payment,
        }
    }

    #[must_use]
    pub const fn shape(&self) -> HandShape {
        self.shape
    }

    #[must_use]
    pub const fn wait(&self) -> WaitKind {
        self.wait
    }

    #[must_use]
    pub fn yaku(&self) -> &[YakuValue] {
        &self.yaku
    }

    #[must_use]
    pub const fn bonuses(&self) -> BonusHan {
        self.bonuses
    }

    #[must_use]
    pub const fn han(&self) -> u8 {
        self.han
    }

    #[must_use]
    pub const fn fu(&self) -> u16 {
        self.fu
    }

    #[must_use]
    pub const fn yakuman_multiplier(&self) -> u8 {
        self.yakuman_multiplier
    }

    #[must_use]
    pub const fn base_points(&self) -> u32 {
        self.base_points
    }

    #[must_use]
    pub const fn limit(&self) -> Limit {
        self.limit
    }

    #[must_use]
    pub const fn payment(&self) -> Payment {
        self.payment
    }
}
