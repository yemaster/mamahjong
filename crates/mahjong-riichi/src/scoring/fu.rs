use crate::{WinQuery, WinSource};

use super::analysis::{CompleteGroup, CompleteGroupKind, Interpretation};
use super::result::{HandShape, WaitKind, Yaku, YakuValue};
use super::yaku::{is_closed, is_value_pair};

pub(super) fn calculate_fu(
    query: WinQuery<'_>,
    interpretation: &Interpretation,
    yaku: &[YakuValue],
) -> u16 {
    if matches!(interpretation.shape, HandShape::SevenPairs) {
        return 25;
    }
    if yaku.iter().any(|value| value.yaku() == Yaku::Pinfu)
        && matches!(query.source(), WinSource::Tsumo(_))
    {
        return 20;
    }

    let mut fu = 20_u16;
    if is_closed(query) && !matches!(query.source(), WinSource::Tsumo(_)) {
        fu += 10;
    }
    if matches!(query.source(), WinSource::Tsumo(_)) {
        fu += 2;
    }
    if interpretation
        .pair
        .is_some_and(|pair| is_value_pair(query, pair))
    {
        fu += 2;
    }
    if matches!(
        interpretation.wait,
        WaitKind::Edge | WaitKind::Closed | WaitKind::Pair
    ) {
        fu += 2;
    }
    fu += interpretation.groups.iter().map(group_fu).sum::<u16>();

    if fu == 20 && !is_closed(query) {
        return 30;
    }
    round_up(fu, 10)
}

fn group_fu(group: &CompleteGroup) -> u16 {
    let (kind, kan) = match group.kind {
        CompleteGroupKind::Sequence(_) => return 0,
        CompleteGroupKind::Triplet(kind) => (kind, false),
        CompleteGroupKind::Kan(kind) => (kind, true),
    };
    let terminal_or_honor = kind.is_terminal_or_honor();
    match (kan, group.open, terminal_or_honor) {
        (false, true, false) => 2,
        (false, true, true) => 4,
        (false, false, false) => 4,
        (false, false, true) => 8,
        (true, true, false) => 8,
        (true, true, true) => 16,
        (true, false, false) => 16,
        (true, false, true) => 32,
    }
}

const fn round_up(value: u16, unit: u16) -> u16 {
    value.div_ceil(unit) * unit
}

#[cfg(test)]
mod tests {
    use super::round_up;

    #[test]
    fn rounds_fu_to_next_ten() {
        assert_eq!(round_up(22, 10), 30);
        assert_eq!(round_up(30, 10), 30);
    }
}
