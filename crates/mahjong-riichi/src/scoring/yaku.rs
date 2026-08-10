use std::collections::BTreeMap;

use crate::{
    DrawSource, Honor, MeldKind, RiichiStatus, Suit, TileKind, WinQuery, WinSource, Wind,
    YakumanValue,
};

use super::analysis::{CompleteGroup, CompleteGroupKind, Interpretation};
use super::result::{BonusHan, HandShape, WaitKind, Yaku, YakuValue};

pub(super) fn yakuman(query: WinQuery<'_>, interpretation: &Interpretation) -> Vec<YakuValue> {
    let mut values = Vec::new();
    let double_variants = matches!(
        query.rules().scoring.yakuman_value,
        YakumanValue::DoubleVariantsAndStacked
    );
    let closed = is_closed(query);
    let winner_is_dealer = query.seat() == query.progress().dealer();
    let first_uninterrupted_turn = !query.calls_occurred() && query.player().discards().is_empty();

    if matches!(query.source(), WinSource::Tsumo(_)) && first_uninterrupted_turn {
        values.push(YakuValue::yakuman(
            if winner_is_dealer {
                Yaku::Tenhou
            } else {
                Yaku::Chiihou
            },
            1,
        ));
    }
    if query.rules().scoring.old_yaku
        && !winner_is_dealer
        && first_uninterrupted_turn
        && matches!(query.source(), WinSource::Discard { .. })
    {
        values.push(YakuValue::yakuman(Yaku::Renhou, 1));
    }

    if matches!(interpretation.shape, HandShape::ThirteenOrphans) {
        values.push(YakuValue::yakuman(
            if matches!(interpretation.wait, WaitKind::ThirteenSided) {
                Yaku::ThirteenWaitOrphans
            } else {
                Yaku::ThirteenOrphans
            },
            if double_variants && matches!(interpretation.wait, WaitKind::ThirteenSided) {
                2
            } else {
                1
            },
        ));
    }

    if closed {
        if let Some(pure) = nine_gates(interpretation, query.winning_tile().kind()) {
            values.push(YakuValue::yakuman(
                if pure {
                    Yaku::PureChuurenPoutou
                } else {
                    Yaku::ChuurenPoutou
                },
                if pure && double_variants { 2 } else { 1 },
            ));
        }
    }

    if matches!(interpretation.shape, HandShape::Standard) {
        let triplets: Vec<_> = interpretation
            .groups
            .iter()
            .filter(|group| {
                matches!(
                    group.kind,
                    CompleteGroupKind::Triplet(_) | CompleteGroupKind::Kan(_)
                )
            })
            .collect();
        let concealed_triplets = triplets.iter().filter(|group| !group.open).count();
        if concealed_triplets == 4 {
            let tanki = matches!(interpretation.wait, WaitKind::Pair);
            if tanki || matches!(query.source(), WinSource::Tsumo(_)) {
                values.push(YakuValue::yakuman(
                    if tanki {
                        Yaku::SuuankouTanki
                    } else {
                        Yaku::Suuankou
                    },
                    if tanki && double_variants { 2 } else { 1 },
                ));
            }
        }

        let dragon_triplets = triplets
            .iter()
            .filter(|group| group_kind(group).is_some_and(is_dragon))
            .count();
        if dragon_triplets == 3 {
            values.push(YakuValue::yakuman(Yaku::Daisangen, 1));
        }
        let wind_triplets = triplets
            .iter()
            .filter(|group| group_kind(group).is_some_and(is_wind))
            .count();
        if wind_triplets == 4 {
            values.push(YakuValue::yakuman(
                Yaku::Daisuushi,
                if double_variants { 2 } else { 1 },
            ));
        } else if wind_triplets == 3 && interpretation.pair.is_some_and(is_wind) {
            values.push(YakuValue::yakuman(Yaku::Shousuushi, 1));
        }
        if interpretation
            .groups
            .iter()
            .filter(|group| matches!(group.kind, CompleteGroupKind::Kan(_)))
            .count()
            == 4
        {
            values.push(YakuValue::yakuman(Yaku::Suukantsu, 1));
        }
    }

    if interpretation
        .all_tiles
        .iter()
        .all(|tile| tile.kind().is_honor())
    {
        values.push(YakuValue::yakuman(Yaku::Tsuuiisou, 1));
    }
    if interpretation
        .all_tiles
        .iter()
        .all(|tile| tile.kind().is_terminal())
    {
        values.push(YakuValue::yakuman(Yaku::Chinroutou, 1));
    }
    if interpretation
        .all_tiles
        .iter()
        .all(|tile| is_green(tile.kind()))
    {
        values.push(YakuValue::yakuman(Yaku::Ryuuiisou, 1));
    }
    if query.rules().scoring.old_yaku && matches!(interpretation.shape, HandShape::SevenPairs) {
        if let Some(yaku) = wheel_yaku(interpretation) {
            values.push(YakuValue::yakuman(yaku, 1));
        }
    }

    values.sort_unstable_by_key(|value| value.yaku());
    values
}

pub(super) fn regular_yaku(query: WinQuery<'_>, interpretation: &Interpretation) -> Vec<YakuValue> {
    let mut values = Vec::new();
    let closed = is_closed(query);
    add_context_yaku(&mut values, query, closed);

    if (closed || query.rules().calls.kuitan)
        && interpretation
            .all_tiles
            .iter()
            .all(|tile| !tile.kind().is_terminal_or_honor())
    {
        values.push(YakuValue::han(Yaku::Tanyao, 1));
    }
    add_flush_yaku(&mut values, interpretation, closed);

    if matches!(interpretation.shape, HandShape::SevenPairs) {
        values.push(YakuValue::han(Yaku::SevenPairs, 2));
        if interpretation
            .all_tiles
            .iter()
            .all(|tile| tile.kind().is_terminal_or_honor())
        {
            values.push(YakuValue::han(Yaku::Honroutou, 2));
        }
        values.sort_unstable_by_key(|value| value.yaku());
        return values;
    }
    if !matches!(interpretation.shape, HandShape::Standard) {
        values.sort_unstable_by_key(|value| value.yaku());
        return values;
    }

    let groups = &interpretation.groups;
    let triplets: Vec<_> = groups
        .iter()
        .filter(|group| {
            matches!(
                group.kind,
                CompleteGroupKind::Triplet(_) | CompleteGroupKind::Kan(_)
            )
        })
        .collect();
    let sequences: Vec<_> = groups
        .iter()
        .filter_map(|group| match group.kind {
            CompleteGroupKind::Sequence(start) => Some(start),
            CompleteGroupKind::Triplet(_) | CompleteGroupKind::Kan(_) => None,
        })
        .collect();

    add_value_tile_yaku(&mut values, query, &triplets);
    if closed
        && sequences.len() == 4
        && interpretation
            .pair
            .is_some_and(|pair| !is_value_pair(query, pair))
        && matches!(interpretation.wait, WaitKind::TwoSided)
    {
        values.push(YakuValue::han(Yaku::Pinfu, 1));
    }

    if closed {
        let sequence_counts = occurrence_counts(sequences.iter().copied());
        let pairs = sequence_counts.values().map(|count| count / 2).sum::<u8>();
        if pairs >= 2 {
            values.push(YakuValue::han(Yaku::Ryanpeikou, 3));
        } else if pairs == 1 {
            values.push(YakuValue::han(Yaku::Iipeikou, 1));
        }
    }
    if sequences.is_empty() {
        values.push(YakuValue::han(Yaku::Toitoi, 2));
    }
    if triplets.iter().filter(|group| !group.open).count() >= 3 {
        values.push(YakuValue::han(Yaku::Sanankou, 2));
    }
    if groups
        .iter()
        .filter(|group| matches!(group.kind, CompleteGroupKind::Kan(_)))
        .count()
        >= 3
    {
        values.push(YakuValue::han(Yaku::Sankantsu, 2));
    }
    if has_sanshoku_triplets(&triplets) {
        values.push(YakuValue::han(Yaku::SanshokuDoukou, 2));
    }
    if has_sanshoku_sequences(&sequences) {
        values.push(YakuValue::han(
            Yaku::SanshokuDoujun,
            if closed { 2 } else { 1 },
        ));
    }
    if has_ittsu(&sequences) {
        values.push(YakuValue::han(Yaku::Ittsu, if closed { 2 } else { 1 }));
    }

    let dragon_triplets = triplets
        .iter()
        .filter(|group| group_kind(group).is_some_and(is_dragon))
        .count();
    if dragon_triplets == 2 && interpretation.pair.is_some_and(is_dragon) {
        values.push(YakuValue::han(Yaku::Shousangen, 2));
    }
    if interpretation
        .all_tiles
        .iter()
        .all(|tile| tile.kind().is_terminal_or_honor())
    {
        values.push(YakuValue::han(Yaku::Honroutou, 2));
    }

    add_outside_hand_yaku(&mut values, interpretation, &sequences, closed);
    values.sort_unstable_by_key(|value| value.yaku());
    values
}

pub(super) fn bonus_han(query: WinQuery<'_>, interpretation: &Interpretation) -> BonusHan {
    let dora = query
        .wall()
        .current_dora_indicators()
        .map(|indicator| count_kind(interpretation, dora_after(indicator.kind())))
        .sum();
    let ura_dora = if query.rules().bonuses.ura_dora
        && matches!(query.player().riichi_status(), RiichiStatus::Established)
    {
        query
            .wall()
            .matching_ura_dora_indicators()
            .map(|indicator| count_kind(interpretation, dora_after(indicator.kind())))
            .sum()
    } else {
        0
    };
    let red_dora = u8::try_from(
        interpretation
            .all_tiles
            .iter()
            .filter(|tile| tile.is_red())
            .count(),
    )
    .expect("hand red tile count fits u8");
    BonusHan::new(dora, ura_dora, red_dora)
}

pub(super) fn is_closed(query: WinQuery<'_>) -> bool {
    query
        .player()
        .melds()
        .iter()
        .all(|meld| matches!(meld.kind(), MeldKind::ConcealedKan))
}

pub(super) fn is_value_pair(query: WinQuery<'_>, pair: TileKind) -> bool {
    is_dragon(pair)
        || wind_kind(query.progress().round_wind()) == pair
        || query
            .progress()
            .seat_wind(query.seat())
            .is_ok_and(|wind| wind_kind(wind) == pair)
}

fn add_context_yaku(values: &mut Vec<YakuValue>, query: WinQuery<'_>, closed: bool) {
    if closed && matches!(query.player().riichi_status(), RiichiStatus::Established) {
        values.push(YakuValue::han(
            if query.player().is_double_riichi() {
                Yaku::DoubleRiichi
            } else {
                Yaku::Riichi
            },
            if query.player().is_double_riichi() {
                2
            } else {
                1
            },
        ));
        if query.player().is_ippatsu_eligible() {
            values.push(YakuValue::han(Yaku::Ippatsu, 1));
        }
    }
    match query.source() {
        WinSource::Tsumo(source) => {
            if closed {
                values.push(YakuValue::han(Yaku::MenzenTsumo, 1));
            }
            match source {
                DrawSource::Rinshan => values.push(YakuValue::han(Yaku::Rinshan, 1)),
                DrawSource::LiveWall if query.wall().remaining_live_draws() == 0 => {
                    values.push(YakuValue::han(Yaku::Haitei, 1));
                }
                DrawSource::LiveWall => {}
            }
        }
        WinSource::Discard { .. } if query.wall().remaining_live_draws() == 0 => {
            values.push(YakuValue::han(Yaku::Houtei, 1));
        }
        WinSource::AddedKan { .. } | WinSource::ConcealedKan { .. } => {
            values.push(YakuValue::han(Yaku::Chankan, 1));
        }
        WinSource::Discard { .. } => {}
    }
}

fn add_value_tile_yaku(
    values: &mut Vec<YakuValue>,
    query: WinQuery<'_>,
    triplets: &[&CompleteGroup],
) {
    for group in triplets {
        let Some(kind) = group_kind(group) else {
            continue;
        };
        match kind.honor_value() {
            Some(Honor::White) => values.push(YakuValue::han(Yaku::WhiteDragon, 1)),
            Some(Honor::Green) => values.push(YakuValue::han(Yaku::GreenDragon, 1)),
            Some(Honor::Red) => values.push(YakuValue::han(Yaku::RedDragon, 1)),
            Some(_) | None => {}
        }
        if kind == wind_kind(query.progress().round_wind()) {
            values.push(YakuValue::han(Yaku::RoundWind, 1));
        }
        if query
            .progress()
            .seat_wind(query.seat())
            .is_ok_and(|wind| kind == wind_kind(wind))
        {
            values.push(YakuValue::han(Yaku::SeatWind, 1));
        }
    }
}

fn add_flush_yaku(values: &mut Vec<YakuValue>, interpretation: &Interpretation, closed: bool) {
    let suits: Vec<_> = interpretation
        .all_tiles
        .iter()
        .filter_map(|tile| tile.kind().suit())
        .collect();
    let Some(first_suit) = suits.first().copied() else {
        return;
    };
    if suits.iter().all(|suit| *suit == first_suit) {
        if interpretation
            .all_tiles
            .iter()
            .any(|tile| tile.kind().is_honor())
        {
            values.push(YakuValue::han(Yaku::Honitsu, if closed { 3 } else { 2 }));
        } else {
            values.push(YakuValue::han(Yaku::Chinitsu, if closed { 6 } else { 5 }));
        }
    }
}

fn add_outside_hand_yaku(
    values: &mut Vec<YakuValue>,
    interpretation: &Interpretation,
    sequences: &[TileKind],
    closed: bool,
) {
    if sequences.is_empty()
        || !interpretation
            .pair
            .is_some_and(TileKind::is_terminal_or_honor)
        || !interpretation
            .groups
            .iter()
            .all(group_has_terminal_or_honor)
    {
        return;
    }
    let has_honor = interpretation
        .all_tiles
        .iter()
        .any(|tile| tile.kind().is_honor());
    values.push(YakuValue::han(
        if has_honor {
            Yaku::Chanta
        } else {
            Yaku::Junchan
        },
        match (has_honor, closed) {
            (true, true) => 2,
            (true, false) => 1,
            (false, true) => 3,
            (false, false) => 2,
        },
    ));
}

fn occurrence_counts(values: impl IntoIterator<Item = TileKind>) -> BTreeMap<TileKind, u8> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
}

fn has_sanshoku_sequences(sequences: &[TileKind]) -> bool {
    (1..=7).any(|rank| {
        [Suit::Man, Suit::Pin, Suit::Sou].into_iter().all(|suit| {
            let start = TileKind::suited(suit, crate::Rank::new(rank).expect("rank"));
            sequences.contains(&start)
        })
    })
}

fn has_ittsu(sequences: &[TileKind]) -> bool {
    [Suit::Man, Suit::Pin, Suit::Sou].into_iter().any(|suit| {
        [1, 4, 7].into_iter().all(|rank| {
            sequences.contains(&TileKind::suited(
                suit,
                crate::Rank::new(rank).expect("rank"),
            ))
        })
    })
}

fn has_sanshoku_triplets(triplets: &[&CompleteGroup]) -> bool {
    (1..=9).any(|rank| {
        [Suit::Man, Suit::Pin, Suit::Sou].into_iter().all(|suit| {
            let kind = TileKind::suited(suit, crate::Rank::new(rank).expect("rank"));
            triplets.iter().any(|group| group_kind(group) == Some(kind))
        })
    })
}

fn group_has_terminal_or_honor(group: &CompleteGroup) -> bool {
    match group.kind {
        CompleteGroupKind::Sequence(start) => start
            .rank()
            .is_some_and(|rank| rank.value() == 1 || rank.value() == 7),
        CompleteGroupKind::Triplet(kind) | CompleteGroupKind::Kan(kind) => {
            kind.is_terminal_or_honor()
        }
    }
}

fn group_kind(group: &CompleteGroup) -> Option<TileKind> {
    match group.kind {
        CompleteGroupKind::Triplet(kind) | CompleteGroupKind::Kan(kind) => Some(kind),
        CompleteGroupKind::Sequence(_) => None,
    }
}

fn is_dragon(kind: TileKind) -> bool {
    matches!(
        kind.honor_value(),
        Some(Honor::White | Honor::Green | Honor::Red)
    )
}

fn is_wind(kind: TileKind) -> bool {
    matches!(
        kind.honor_value(),
        Some(Honor::East | Honor::South | Honor::West | Honor::North)
    )
}

fn wind_kind(wind: Wind) -> TileKind {
    TileKind::honor(match wind {
        Wind::East => Honor::East,
        Wind::South => Honor::South,
        Wind::West => Honor::West,
        Wind::North => Honor::North,
    })
}

fn is_green(kind: TileKind) -> bool {
    matches!(kind.honor_value(), Some(Honor::Green))
        || kind.suit() == Some(Suit::Sou)
            && kind
                .rank()
                .is_some_and(|rank| matches!(rank.value(), 2 | 3 | 4 | 6 | 8))
}

fn nine_gates(interpretation: &Interpretation, winning_kind: TileKind) -> Option<bool> {
    let suit = interpretation
        .all_tiles
        .first()
        .and_then(|tile| tile.kind().suit())?;
    if interpretation
        .all_tiles
        .iter()
        .any(|tile| tile.kind().suit() != Some(suit))
    {
        return None;
    }
    let pattern = [3, 1, 1, 1, 1, 1, 1, 1, 3];
    let valid = pattern.iter().enumerate().all(|(offset, minimum)| {
        let kind = TileKind::suited(
            suit,
            crate::Rank::new(u8::try_from(offset + 1).expect("rank")).expect("rank"),
        );
        interpretation.concealed_counts.get(kind) >= *minimum
    });
    if !valid {
        return None;
    }
    let mut pre_pattern = true;
    for (offset, expected) in pattern.into_iter().enumerate() {
        let kind = TileKind::suited(
            suit,
            crate::Rank::new(u8::try_from(offset + 1).expect("rank")).expect("rank"),
        );
        if interpretation.pre_win_counts.get(kind) != expected {
            pre_pattern = false;
            break;
        }
    }
    Some(pre_pattern && winning_kind.suit() == Some(suit))
}

fn wheel_yaku(interpretation: &Interpretation) -> Option<Yaku> {
    let suit = interpretation
        .all_tiles
        .first()
        .and_then(|tile| tile.kind().suit())?;
    let exact = (2..=8).all(|rank| {
        interpretation.concealed_counts.get(TileKind::suited(
            suit,
            crate::Rank::new(rank).expect("rank"),
        )) == 2
    }) && interpretation
        .all_tiles
        .iter()
        .all(|tile| tile.kind().suit() == Some(suit));
    exact.then_some(match suit {
        Suit::Pin => Yaku::Daisharin,
        Suit::Sou => Yaku::Daichikurin,
        Suit::Man => Yaku::Daisuurin,
    })
}

fn dora_after(indicator: TileKind) -> TileKind {
    if let (Some(suit), Some(rank)) = (indicator.suit(), indicator.rank()) {
        let next = if rank.value() == 9 {
            1
        } else {
            rank.value() + 1
        };
        return TileKind::suited(suit, crate::Rank::new(next).expect("wrapped rank"));
    }
    TileKind::honor(match indicator.honor_value().expect("honor indicator") {
        Honor::East => Honor::South,
        Honor::South => Honor::West,
        Honor::West => Honor::North,
        Honor::North => Honor::East,
        Honor::White => Honor::Green,
        Honor::Green => Honor::Red,
        Honor::Red => Honor::White,
    })
}

fn count_kind(interpretation: &Interpretation, kind: TileKind) -> u8 {
    u8::try_from(
        interpretation
            .all_tiles
            .iter()
            .filter(|tile| tile.kind() == kind)
            .count(),
    )
    .expect("hand tile count fits u8")
}
