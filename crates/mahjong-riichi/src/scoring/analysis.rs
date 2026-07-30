use crate::{MeldKind, Tile, TileKind, WinQuery, WinSource};

use super::counts::TileCounts;
use super::result::{HandShape, WaitKind};
use super::shape::{Group, StandardShape, WinningShape, winning_shapes};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompleteGroupKind {
    Sequence(TileKind),
    Triplet(TileKind),
    Kan(TileKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CompleteGroup {
    pub(super) kind: CompleteGroupKind,
    pub(super) open: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Interpretation {
    pub(super) shape: HandShape,
    pub(super) wait: WaitKind,
    pub(super) pair: Option<TileKind>,
    pub(super) groups: Box<[CompleteGroup]>,
    pub(super) all_tiles: Box<[Tile]>,
    pub(super) pre_win_counts: TileCounts,
    pub(super) concealed_counts: TileCounts,
}

pub(super) fn interpretations(query: WinQuery<'_>) -> Box<[Interpretation]> {
    let player = query.player();
    let mut concealed = player.concealed().to_vec();
    if !matches!(query.source(), WinSource::Tsumo(_)) {
        concealed.push(query.winning_tile());
    }
    let Ok(concealed_counts) = TileCounts::from_tiles(&concealed) else {
        return Box::new([]);
    };
    let mut pre_win_counts = concealed_counts.clone();
    if !pre_win_counts.remove(query.winning_tile().kind()) {
        return Box::new([]);
    }

    let mut all_tiles = concealed.clone();
    all_tiles.extend(
        player
            .melds()
            .iter()
            .flat_map(|meld| meld.tiles().iter().copied()),
    );
    let declared_groups: Vec<_> = player
        .melds()
        .iter()
        .map(|meld| {
            let kind = match meld.kind() {
                MeldKind::Chi => {
                    let start = meld
                        .tiles()
                        .iter()
                        .map(|tile| tile.kind())
                        .min()
                        .expect("meld has tiles");
                    CompleteGroupKind::Sequence(start)
                }
                MeldKind::Pon => CompleteGroupKind::Triplet(meld.tile_kind()),
                MeldKind::OpenKan | MeldKind::ConcealedKan | MeldKind::AddedKan => {
                    CompleteGroupKind::Kan(meld.tile_kind())
                }
            };
            CompleteGroup {
                kind,
                open: !matches!(meld.kind(), MeldKind::ConcealedKan),
            }
        })
        .collect();

    let mut results = Vec::new();
    for shape in winning_shapes(&concealed_counts, declared_groups.len()).iter() {
        match shape {
            WinningShape::Standard(standard) => append_standard_interpretations(
                &mut results,
                standard,
                &declared_groups,
                &all_tiles,
                &pre_win_counts,
                &concealed_counts,
                query,
            ),
            WinningShape::SevenPairs => results.push(Interpretation {
                shape: HandShape::SevenPairs,
                wait: WaitKind::Pair,
                pair: None,
                groups: Box::new([]),
                all_tiles: all_tiles.clone().into_boxed_slice(),
                pre_win_counts: pre_win_counts.clone(),
                concealed_counts: concealed_counts.clone(),
            }),
            WinningShape::ThirteenOrphans => {
                let thirteen_sided = pre_win_counts
                    .iter()
                    .filter(|(kind, _)| kind.is_terminal_or_honor())
                    .all(|(_, count)| count == 1);
                results.push(Interpretation {
                    shape: HandShape::ThirteenOrphans,
                    wait: if thirteen_sided {
                        WaitKind::ThirteenSided
                    } else {
                        WaitKind::Pair
                    },
                    pair: None,
                    groups: Box::new([]),
                    all_tiles: all_tiles.clone().into_boxed_slice(),
                    pre_win_counts: pre_win_counts.clone(),
                    concealed_counts: concealed_counts.clone(),
                });
            }
        }
    }
    results.into_boxed_slice()
}

#[allow(clippy::too_many_arguments)]
fn append_standard_interpretations(
    results: &mut Vec<Interpretation>,
    standard: &StandardShape,
    declared_groups: &[CompleteGroup],
    all_tiles: &[Tile],
    pre_win_counts: &TileCounts,
    concealed_counts: &TileCounts,
    query: WinQuery<'_>,
) {
    let winning_kind = query.winning_tile().kind();
    if standard.pair == winning_kind {
        push_standard(
            results,
            standard,
            declared_groups,
            all_tiles,
            pre_win_counts,
            concealed_counts,
            WaitKind::Pair,
            None,
        );
    }
    for (index, group) in standard.groups.iter().enumerate() {
        match group {
            Group::Triplet(kind) if *kind == winning_kind => push_standard(
                results,
                standard,
                declared_groups,
                all_tiles,
                pre_win_counts,
                concealed_counts,
                WaitKind::DoublePair,
                (!matches!(query.source(), WinSource::Tsumo(_))).then_some(index),
            ),
            Group::Sequence(start) if sequence_contains(*start, winning_kind) => push_standard(
                results,
                standard,
                declared_groups,
                all_tiles,
                pre_win_counts,
                concealed_counts,
                sequence_wait(*start, winning_kind),
                None,
            ),
            Group::Sequence(_) | Group::Triplet(_) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_standard(
    results: &mut Vec<Interpretation>,
    standard: &StandardShape,
    declared_groups: &[CompleteGroup],
    all_tiles: &[Tile],
    pre_win_counts: &TileCounts,
    concealed_counts: &TileCounts,
    wait: WaitKind,
    ron_completed_index: Option<usize>,
) {
    let mut groups = Vec::with_capacity(4);
    groups.extend(
        standard
            .groups
            .iter()
            .enumerate()
            .map(|(index, group)| CompleteGroup {
                kind: match group {
                    Group::Sequence(start) => CompleteGroupKind::Sequence(*start),
                    Group::Triplet(kind) => CompleteGroupKind::Triplet(*kind),
                },
                open: ron_completed_index == Some(index),
            }),
    );
    groups.extend_from_slice(declared_groups);
    results.push(Interpretation {
        shape: HandShape::Standard,
        wait,
        pair: Some(standard.pair),
        groups: groups.into_boxed_slice(),
        all_tiles: all_tiles.to_vec().into_boxed_slice(),
        pre_win_counts: pre_win_counts.clone(),
        concealed_counts: concealed_counts.clone(),
    });
}

fn sequence_contains(start: TileKind, kind: TileKind) -> bool {
    start.suit() == kind.suit()
        && start.rank().zip(kind.rank()).is_some_and(|(start, kind)| {
            (start.value()..=start.value() + 2).contains(&kind.value())
        })
}

fn sequence_wait(start: TileKind, winning: TileKind) -> WaitKind {
    let start_rank = start.rank().expect("sequence start has rank").value();
    let winning_rank = winning.rank().expect("sequence tile has rank").value();
    if winning_rank == start_rank + 1 {
        WaitKind::Closed
    } else if (start_rank == 1 && winning_rank == 3) || (start_rank == 7 && winning_rank == 7) {
        WaitKind::Edge
    } else {
        WaitKind::TwoSided
    }
}
