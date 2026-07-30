use crate::{RiichiRules, WinSource};

use super::result::{Limit, Payment};

pub(super) fn base_points(
    rules: &RiichiRules,
    han: u8,
    fu: u16,
    yakuman_multiplier: u8,
) -> (u32, Limit) {
    if yakuman_multiplier > 0 {
        return (
            8_000 * u32::from(yakuman_multiplier),
            Limit::Yakuman(yakuman_multiplier),
        );
    }
    if han >= 13 {
        return if rules.scoring.kazoe_yakuman {
            (8_000, Limit::KazoeYakuman)
        } else {
            (6_000, Limit::Sanbaiman)
        };
    }
    if han >= 11 {
        return (6_000, Limit::Sanbaiman);
    }
    if han >= 8 {
        return (4_000, Limit::Baiman);
    }
    if han >= 6 {
        return (3_000, Limit::Haneman);
    }
    if han >= 5
        || rules.scoring.kiriage_mangan && ((han == 4 && fu == 30) || (han == 3 && fu == 60))
    {
        return (2_000, Limit::Mangan);
    }

    let shift = u32::from(han) + 2;
    let calculated = u32::from(fu).checked_shl(shift).unwrap_or(u32::MAX);
    if calculated >= 2_000 {
        (2_000, Limit::Mangan)
    } else {
        (calculated, Limit::None)
    }
}

pub(super) fn payment(base_points: u32, winner_is_dealer: bool, source: WinSource) -> Payment {
    if matches!(source, WinSource::Tsumo(_)) {
        if winner_is_dealer {
            Payment::Tsumo {
                dealer_payment: 0,
                other_payment: round_hundred(base_points * 2),
            }
        } else {
            Payment::Tsumo {
                dealer_payment: round_hundred(base_points * 2),
                other_payment: round_hundred(base_points),
            }
        }
    } else {
        Payment::Ron {
            points: round_hundred(base_points * if winner_is_dealer { 6 } else { 4 }),
        }
    }
}

const fn round_hundred(points: u32) -> u32 {
    points.div_ceil(100) * 100
}

#[cfg(test)]
mod tests {
    use crate::RiichiRules;

    use super::Limit;

    use super::base_points;

    #[test]
    fn applies_normal_and_kiriage_mangan_boundaries() {
        let mut rules = RiichiRules::default();
        rules.scoring.kiriage_mangan = false;
        assert_eq!(base_points(&rules, 4, 30, 0), (1_920, Limit::None));
        assert_eq!(base_points(&rules, 3, 60, 0), (1_920, Limit::None));

        rules.scoring.kiriage_mangan = true;
        assert_eq!(base_points(&rules, 4, 30, 0), (2_000, Limit::Mangan));
        assert_eq!(base_points(&rules, 3, 60, 0), (2_000, Limit::Mangan));
    }

    #[test]
    fn counted_yakuman_respects_configuration() {
        let mut rules = RiichiRules::default();
        rules.scoring.kazoe_yakuman = true;
        assert_eq!(base_points(&rules, 13, 30, 0), (8_000, Limit::KazoeYakuman));
        rules.scoring.kazoe_yakuman = false;
        assert_eq!(base_points(&rules, 13, 30, 0), (6_000, Limit::Sanbaiman));
    }
}
