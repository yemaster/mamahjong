use crate::{Tile, TileKind};

use super::counts::TileCounts;

const STANDARD_GROUP_COUNT: usize = 4;
const ORPHAN_CODES: [&str; 13] = [
    "1m", "9m", "1p", "9p", "1s", "9s", "1z", "2z", "3z", "4z", "5z", "6z", "7z",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Group {
    Sequence(TileKind),
    Triplet(TileKind),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StandardShape {
    pub(super) pair: TileKind,
    pub(super) groups: Box<[Group]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum WinningShape {
    Standard(StandardShape),
    SevenPairs,
    ThirteenOrphans,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WaitingTiles {
    kinds: Box<[TileKind]>,
}

impl WaitingTiles {
    #[must_use]
    pub fn kinds(&self) -> &[TileKind] {
        &self.kinds
    }

    #[must_use]
    pub fn contains(&self, kind: TileKind) -> bool {
        self.kinds.contains(&kind)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

pub(super) fn winning_shapes(
    counts: &TileCounts,
    declared_group_count: usize,
) -> Box<[WinningShape]> {
    if declared_group_count > STANDARD_GROUP_COUNT {
        return Box::new([]);
    }
    let concealed_group_count = STANDARD_GROUP_COUNT - declared_group_count;
    let expected_tiles = concealed_group_count * 3 + 2;
    if usize::from(counts.total()) != expected_tiles {
        return Box::new([]);
    }

    let mut shapes = standard_shapes(counts, concealed_group_count);
    if declared_group_count == 0 && is_seven_pairs(counts) {
        shapes.push(WinningShape::SevenPairs);
    }
    if declared_group_count == 0 && is_thirteen_orphans(counts) {
        shapes.push(WinningShape::ThirteenOrphans);
    }
    shapes.sort_unstable();
    shapes.dedup();
    shapes.into_boxed_slice()
}

pub(super) fn waiting_tiles(concealed: &[Tile], declared_group_count: usize) -> WaitingTiles {
    let Ok(mut counts) = TileCounts::from_tiles(concealed) else {
        return WaitingTiles::default();
    };
    let mut kinds = Vec::with_capacity(13);
    for index in 0..TileKind::COUNT {
        let kind = TileKind::from_index(u8::try_from(index).expect("tile kind index fits u8"))
            .expect("bounded tile kind");
        if counts.get(kind) >= 4 || counts.add(kind).is_err() {
            continue;
        }
        if !winning_shapes(&counts, declared_group_count).is_empty() {
            kinds.push(kind);
        }
        let removed = counts.remove(kind);
        debug_assert!(removed);
    }
    WaitingTiles {
        kinds: kinds.into_boxed_slice(),
    }
}

fn standard_shapes(counts: &TileCounts, concealed_group_count: usize) -> Vec<WinningShape> {
    let mut results = Vec::new();
    for (pair, count) in counts.iter() {
        if count < 2 {
            continue;
        }
        let mut remaining = counts.clone();
        remaining.remove(pair);
        remaining.remove(pair);
        let mut groups = Vec::with_capacity(concealed_group_count);
        collect_groups(
            &mut remaining,
            concealed_group_count,
            &mut groups,
            pair,
            &mut results,
        );
    }
    results
}

fn collect_groups(
    counts: &mut TileCounts,
    groups_left: usize,
    groups: &mut Vec<Group>,
    pair: TileKind,
    results: &mut Vec<WinningShape>,
) {
    if groups_left == 0 {
        if counts.total() == 0 {
            let mut stable_groups = groups.clone();
            stable_groups.sort_unstable();
            results.push(WinningShape::Standard(StandardShape {
                pair,
                groups: stable_groups.into_boxed_slice(),
            }));
        }
        return;
    }
    let Some(first) = counts.first_present() else {
        return;
    };

    if counts.get(first) >= 3 {
        remove_n(counts, first, 3);
        groups.push(Group::Triplet(first));
        collect_groups(counts, groups_left - 1, groups, pair, results);
        groups.pop();
        add_n(counts, first, 3);
    }

    let Some(suit) = first.suit() else {
        return;
    };
    let rank = first.rank().expect("suited tile has rank").value();
    if rank > 7 {
        return;
    }
    let second = TileKind::suited(suit, crate::Rank::new(rank + 1).expect("rank"));
    let third = TileKind::suited(suit, crate::Rank::new(rank + 2).expect("rank"));
    if counts.get(second) == 0 || counts.get(third) == 0 {
        return;
    }
    counts.remove(first);
    counts.remove(second);
    counts.remove(third);
    groups.push(Group::Sequence(first));
    collect_groups(counts, groups_left - 1, groups, pair, results);
    groups.pop();
    counts.add(first).expect("restore first sequence tile");
    counts.add(second).expect("restore second sequence tile");
    counts.add(third).expect("restore third sequence tile");
}

fn remove_n(counts: &mut TileCounts, kind: TileKind, count: usize) {
    for _ in 0..count {
        let removed = counts.remove(kind);
        debug_assert!(removed);
    }
}

fn add_n(counts: &mut TileCounts, kind: TileKind, count: usize) {
    for _ in 0..count {
        counts.add(kind).expect("restoring removed tiles is valid");
    }
}

fn is_seven_pairs(counts: &TileCounts) -> bool {
    counts.iter().filter(|(_, count)| *count == 2).count() == 7
        && counts.iter().all(|(_, count)| count == 0 || count == 2)
}

fn is_thirteen_orphans(counts: &TileCounts) -> bool {
    let orphans: Vec<_> = ORPHAN_CODES
        .into_iter()
        .map(|code| code.parse::<TileKind>().expect("static orphan code"))
        .collect();
    counts
        .iter()
        .all(|(kind, count)| orphans.contains(&kind) || count == 0)
        && orphans.iter().all(|kind| counts.get(*kind) >= 1)
        && orphans.iter().any(|kind| counts.get(*kind) == 2)
}

#[cfg(test)]
mod tests {
    use crate::{Tile, TileId, TileKind};

    use super::{WinningShape, waiting_tiles, winning_shapes};
    use crate::scoring::counts::TileCounts;

    fn tiles(codes: &str) -> Vec<Tile> {
        codes
            .split_whitespace()
            .enumerate()
            .map(|(index, code)| {
                Tile::new(
                    TileId::new(u16::try_from(index).expect("id")),
                    code.parse().expect("tile kind"),
                    false,
                )
                .expect("tile")
            })
            .collect()
    }

    #[test]
    fn enumerates_multiple_standard_decompositions() {
        let tiles = tiles("1m 1m 1m 2m 2m 2m 3m 3m 3m 4m 4m 4m 5m 5m");
        let counts = TileCounts::from_tiles(&tiles).expect("counts");
        let shapes = winning_shapes(&counts, 0);

        assert!(shapes.len() >= 3);
        assert!(
            shapes
                .iter()
                .all(|shape| matches!(shape, WinningShape::Standard(_)))
        );
    }

    #[test]
    fn recognizes_seven_pairs_and_thirteen_orphans() {
        let pairs = tiles("1m 1m 2m 2m 3p 3p 4p 4p 5s 5s 6s 6s 1z 1z");
        let pair_counts = TileCounts::from_tiles(&pairs).expect("counts");
        assert!(winning_shapes(&pair_counts, 0).contains(&WinningShape::SevenPairs));

        let orphans = tiles("1m 1m 9m 1p 9p 1s 9s 1z 2z 3z 4z 5z 6z 7z");
        let orphan_counts = TileCounts::from_tiles(&orphans).expect("counts");
        assert!(winning_shapes(&orphan_counts, 0).contains(&WinningShape::ThirteenOrphans));
    }

    #[test]
    fn rejects_special_shapes_with_declared_groups() {
        let pairs = tiles("1m 1m 2m 2m 3p 3p 4p 4p 5s 5s 6s");
        let counts = TileCounts::from_tiles(&pairs).expect("counts");

        assert!(winning_shapes(&counts, 1).is_empty());
    }

    #[test]
    fn finds_all_waits_without_counting_a_fifth_tile() {
        let hand = tiles("1m 1m 1m 2m 3m 4m 5m 6m 7m 8m 9m 9m 9m");
        let waits = waiting_tiles(&hand, 0);

        let expected: Vec<TileKind> = ["1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m"]
            .into_iter()
            .map(|code| code.parse().expect("kind"))
            .collect();
        assert_eq!(waits.kinds(), expected);
    }

    #[test]
    fn meld_count_reduces_required_concealed_groups() {
        let hand = tiles("2p 3p 4p 5s");
        let waits = waiting_tiles(&hand, 3);

        assert!(waits.contains("5s".parse().expect("kind")));
    }
}
