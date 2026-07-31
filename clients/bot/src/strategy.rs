use std::cmp::Ordering;
use std::collections::HashSet;

use serde_json::{Value, json};

use crate::model::{MatchView, MeldView, ReactionOptionView, TileView};
use crate::runner::Variant;

const TILE_KIND_COUNT: usize = 34;
const TERMINAL_AND_HONOR_KINDS: [usize; 13] = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BotCommand {
    pub name: &'static str,
    pub payload: Option<Value>,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscardDecision {
    pub tile_id: u16,
    pub tile_code: String,
    pub shanten: i8,
    pub ukeire: u16,
    pub effective_kinds: u8,
}

pub fn turn_command(view: &MatchView, variant: Variant) -> Result<BotCommand, String> {
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "观察者手牌不可见".to_owned())?;
    let counts = tile_counts(tiles)?;
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;

    if shanten(&counts, fixed_melds) == -1 {
        return Ok(BotCommand {
            name: "riichi.tsumo",
            payload: None,
            description: "自摸".to_owned(),
        });
    }

    if player.riichi_status == "established" {
        let tile_id = player
            .drawn_tile_id
            .ok_or_else(|| "立直后没有摸牌标记".to_owned())?;
        let tile = tiles
            .iter()
            .find(|tile| tile.id == tile_id)
            .ok_or_else(|| "摸牌不在手牌中".to_owned())?;
        return Ok(discard_command(
            "riichi.discard",
            DiscardDecision {
                tile_id,
                tile_code: tile.code.clone(),
                shanten: shanten_after_discard(tiles, tile_id, fixed_melds)?,
                ukeire: 0,
                effective_kinds: 0,
            },
        ));
    }

    let visible = visible_counts(view)?;
    let decision = best_discard(tiles, fixed_melds, &visible, variant)?;
    let closed = is_closed(&player.melds);
    let declare_riichi =
        closed && player.points >= 1_000 && player.riichi_status == "none" && decision.shanten == 0;
    Ok(discard_command(
        if declare_riichi {
            "riichi.riichi_discard"
        } else {
            "riichi.discard"
        },
        decision,
    ))
}

pub fn fallback_discard(view: &MatchView, variant: Variant) -> Result<BotCommand, String> {
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "观察者手牌不可见".to_owned())?;
    let visible = visible_counts(view)?;
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;
    Ok(discard_command(
        "riichi.discard",
        best_discard(tiles, fixed_melds, &visible, variant)?,
    ))
}

pub fn reaction_command(view: &MatchView, variant: Variant) -> Result<Option<BotCommand>, String> {
    if view
        .available_reactions
        .iter()
        .any(|reaction| matches!(reaction, ReactionOptionView::Ron))
    {
        return Ok(Some(BotCommand {
            name: "riichi.ron",
            payload: None,
            description: "荣和".to_owned(),
        }));
    }

    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "观察者手牌不可见".to_owned())?;
    let counts = tile_counts(tiles)?;
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;
    let baseline = shanten(&counts, fixed_melds);
    let visible = visible_counts(view)?;
    let mut candidates = Vec::new();

    for reaction in &view.available_reactions {
        let candidate = match reaction {
            ReactionOptionView::Ron => continue,
            ReactionOptionView::Chi { tile_ids } => call_candidate(
                "riichi.chi",
                "吃",
                tile_ids,
                tiles,
                fixed_melds,
                &visible,
                variant,
                1,
            )?,
            ReactionOptionView::Pon { tile_ids } => call_candidate(
                "riichi.pon",
                "碰",
                tile_ids,
                tiles,
                fixed_melds,
                &visible,
                variant,
                3,
            )?,
            ReactionOptionView::OpenKan { tile_ids } => {
                kan_candidate(tile_ids, tiles, fixed_melds, &visible, variant)?
            }
        };
        candidates.push(candidate);
    }

    let selected = candidates.into_iter().min_by(compare_calls);
    if let Some(selected) = selected
        && selected.shanten < baseline
    {
        return Ok(Some(BotCommand {
            name: selected.name,
            payload: Some(json!({"tile_ids": selected.tile_ids})),
            description: format!(
                "{}（{}向听，受入{}枚）",
                selected.label, selected.shanten, selected.ukeire
            ),
        }));
    }

    Ok(Some(BotCommand {
        name: "riichi.pass",
        payload: None,
        description: "过".to_owned(),
    }))
}

pub fn best_discard(
    tiles: &[TileView],
    fixed_melds: u8,
    visible: &[u8; TILE_KIND_COUNT],
    variant: Variant,
) -> Result<DiscardDecision, String> {
    if tiles.is_empty() {
        return Err("手牌为空".to_owned());
    }
    let counts = tile_counts(tiles)?;
    let mut candidates = Vec::with_capacity(tiles.len());
    for tile in tiles {
        let kind = tile_kind(&tile.code)?;
        let mut after = counts;
        after[kind] = after[kind]
            .checked_sub(1)
            .ok_or_else(|| "牌张计数不一致".to_owned())?;
        let base_shanten = shanten(&after, fixed_melds);
        let (ukeire, effective_kinds) =
            acceptance(&after, fixed_melds, base_shanten, visible, variant);
        candidates.push((
            DiscardDecision {
                tile_id: tile.id,
                tile_code: tile.code.clone(),
                shanten: base_shanten,
                ukeire,
                effective_kinds,
            },
            connection_score(&counts, kind),
            is_red(&tile.code),
            kind,
        ));
    }
    candidates.sort_by(compare_discards);
    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.0)
        .ok_or_else(|| "没有可打出的牌".to_owned())
}

pub fn shanten(counts: &[u8; TILE_KIND_COUNT], fixed_melds: u8) -> i8 {
    let regular = regular_shanten(counts, fixed_melds);
    if fixed_melds > 0 {
        return regular;
    }
    regular
        .min(seven_pairs_shanten(counts))
        .min(thirteen_orphans_shanten(counts))
}

fn regular_shanten(counts: &[u8; TILE_KIND_COUNT], fixed_melds: u8) -> i8 {
    let mut mutable = *counts;
    let mut best = 8;
    regular_search(&mut mutable, 0, fixed_melds, false, 0, &mut best);
    best
}

fn regular_search(
    counts: &mut [u8; TILE_KIND_COUNT],
    mut index: usize,
    melds: u8,
    has_pair: bool,
    incomplete: u8,
    best: &mut i8,
) {
    while index < TILE_KIND_COUNT && counts[index] == 0 {
        index += 1;
    }
    if index == TILE_KIND_COUNT {
        let incomplete = incomplete.min(4_u8.saturating_sub(melds));
        let value =
            8 - i8::try_from(melds * 2 + incomplete + u8::from(has_pair)).expect("small counts");
        *best = (*best).min(value);
        return;
    }

    let unused = counts[index];
    counts[index] = 0;
    regular_search(counts, index + 1, melds, has_pair, incomplete, best);
    counts[index] = unused;

    if melds < 4 && counts[index] >= 3 {
        counts[index] -= 3;
        regular_search(counts, index, melds + 1, has_pair, incomplete, best);
        counts[index] += 3;
    }
    if melds < 4 && index < 27 && index % 9 <= 6 && counts[index + 1] > 0 && counts[index + 2] > 0 {
        counts[index] -= 1;
        counts[index + 1] -= 1;
        counts[index + 2] -= 1;
        regular_search(counts, index, melds + 1, has_pair, incomplete, best);
        counts[index] += 1;
        counts[index + 1] += 1;
        counts[index + 2] += 1;
    }
    if counts[index] >= 2 {
        counts[index] -= 2;
        if !has_pair {
            regular_search(counts, index, melds, true, incomplete, best);
        }
        if incomplete < 4 {
            regular_search(counts, index, melds, has_pair, incomplete + 1, best);
        }
        counts[index] += 2;
    }
    if incomplete < 4 && index < 27 && index % 9 <= 7 && counts[index + 1] > 0 {
        counts[index] -= 1;
        counts[index + 1] -= 1;
        regular_search(counts, index, melds, has_pair, incomplete + 1, best);
        counts[index] += 1;
        counts[index + 1] += 1;
    }
    if incomplete < 4 && index < 27 && index % 9 <= 6 && counts[index + 2] > 0 {
        counts[index] -= 1;
        counts[index + 2] -= 1;
        regular_search(counts, index, melds, has_pair, incomplete + 1, best);
        counts[index] += 1;
        counts[index + 2] += 1;
    }
}

fn seven_pairs_shanten(counts: &[u8; TILE_KIND_COUNT]) -> i8 {
    let pairs = counts.iter().filter(|count| **count >= 2).count();
    let distinct = counts.iter().filter(|count| **count > 0).count();
    6 - i8::try_from(pairs).expect("34 kinds")
        + i8::try_from(7_usize.saturating_sub(distinct)).expect("at most seven")
}

fn thirteen_orphans_shanten(counts: &[u8; TILE_KIND_COUNT]) -> i8 {
    let unique = TERMINAL_AND_HONOR_KINDS
        .iter()
        .filter(|kind| counts[**kind] > 0)
        .count();
    let pair = TERMINAL_AND_HONOR_KINDS
        .iter()
        .any(|kind| counts[*kind] >= 2);
    13 - i8::try_from(unique).expect("13 kinds") - i8::from(pair)
}

fn acceptance(
    counts: &[u8; TILE_KIND_COUNT],
    fixed_melds: u8,
    base_shanten: i8,
    visible: &[u8; TILE_KIND_COUNT],
    variant: Variant,
) -> (u16, u8) {
    let mut total = 0_u16;
    let mut kinds = 0_u8;
    for kind in variant.tile_kinds() {
        if counts[kind] >= 4 {
            continue;
        }
        let mut improved = *counts;
        improved[kind] += 1;
        if shanten(&improved, fixed_melds) < base_shanten {
            let remaining = 4_u8.saturating_sub(visible[kind]);
            if remaining > 0 {
                total += u16::from(remaining);
                kinds += 1;
            }
        }
    }
    (total, kinds)
}

fn visible_counts(view: &MatchView) -> Result<[u8; TILE_KIND_COUNT], String> {
    let mut counts = [0_u8; TILE_KIND_COUNT];
    let mut seen = HashSet::new();
    let mut add = |tile: &TileView| -> Result<(), String> {
        if seen.insert(tile.id) {
            let kind = tile_kind(&tile.code)?;
            counts[kind] = counts[kind].saturating_add(1);
        }
        Ok(())
    };
    for tile in &view.dora_indicators {
        add(tile)?;
    }
    for player in &view.players {
        if let Some(tiles) = &player.concealed_tiles {
            for tile in tiles {
                add(tile)?;
            }
        }
        for meld in &player.melds {
            for tile in &meld.tiles {
                add(tile)?;
            }
        }
        for discard in &player.discards {
            add(&discard.tile)?;
        }
    }
    Ok(counts)
}

fn tile_counts(tiles: &[TileView]) -> Result<[u8; TILE_KIND_COUNT], String> {
    let mut counts = [0_u8; TILE_KIND_COUNT];
    for tile in tiles {
        let kind = tile_kind(&tile.code)?;
        counts[kind] = counts[kind]
            .checked_add(1)
            .ok_or_else(|| "牌张计数溢出".to_owned())?;
    }
    Ok(counts)
}

fn shanten_after_discard(tiles: &[TileView], tile_id: u16, fixed_melds: u8) -> Result<i8, String> {
    let mut after = tile_counts(tiles)?;
    let tile = tiles
        .iter()
        .find(|tile| tile.id == tile_id)
        .ok_or_else(|| "打出的牌不在手牌中".to_owned())?;
    let kind = tile_kind(&tile.code)?;
    after[kind] = after[kind].saturating_sub(1);
    Ok(shanten(&after, fixed_melds))
}

fn tile_kind(code: &str) -> Result<usize, String> {
    let bytes = code.as_bytes();
    if bytes.len() != 2 || !bytes[0].is_ascii_digit() {
        return Err(format!("未知牌码：{code}"));
    }
    let rank = if bytes[0] == b'0' { 5 } else { bytes[0] - b'0' };
    match bytes[1] {
        b'm' | b'p' | b's' if (1..=9).contains(&rank) => {
            let suit = match bytes[1] {
                b'm' => 0,
                b'p' => 1,
                b's' => 2,
                _ => unreachable!(),
            };
            Ok(suit * 9 + usize::from(rank - 1))
        }
        b'z' if (1..=7).contains(&rank) => Ok(27 + usize::from(rank - 1)),
        _ => Err(format!("未知牌码：{code}")),
    }
}

fn is_red(code: &str) -> bool {
    code.as_bytes().first() == Some(&b'0')
}

fn is_closed(melds: &[MeldView]) -> bool {
    melds.iter().all(|meld| meld.kind == "concealed_kan")
}

fn connection_score(counts: &[u8; TILE_KIND_COUNT], kind: usize) -> u16 {
    let mut score = u16::from(counts[kind].saturating_sub(1)) * 6;
    if kind >= 27 {
        return score;
    }
    let rank = kind % 9;
    for (distance, weight) in [(1_usize, 3_u16), (2, 1)] {
        if rank >= distance {
            score += u16::from(counts[kind - distance]) * weight;
        }
        if rank + distance < 9 {
            score += u16::from(counts[kind + distance]) * weight;
        }
    }
    score
}

fn discard_command(name: &'static str, decision: DiscardDecision) -> BotCommand {
    let action = if name == "riichi.riichi_discard" {
        "立直"
    } else {
        "打"
    };
    BotCommand {
        name,
        payload: Some(json!({"tile_id": decision.tile_id})),
        description: format!(
            "{action} {}（{}向听，受入{}枚/{}种）",
            decision.tile_code, decision.shanten, decision.ukeire, decision.effective_kinds
        ),
    }
}

#[derive(Clone, Debug)]
struct CallCandidate {
    name: &'static str,
    label: &'static str,
    tile_ids: Vec<u16>,
    shanten: i8,
    ukeire: u16,
    priority: u8,
}

#[allow(clippy::too_many_arguments)]
fn call_candidate<const N: usize>(
    name: &'static str,
    label: &'static str,
    tile_ids: &[u16; N],
    tiles: &[TileView],
    fixed_melds: u8,
    visible: &[u8; TILE_KIND_COUNT],
    variant: Variant,
    priority: u8,
) -> Result<CallCandidate, String> {
    let remaining = remove_tiles(tiles, tile_ids)?;
    let decision = best_discard(&remaining, fixed_melds + 1, visible, variant)?;
    Ok(CallCandidate {
        name,
        label,
        tile_ids: tile_ids.to_vec(),
        shanten: decision.shanten,
        ukeire: decision.ukeire,
        priority,
    })
}

fn kan_candidate(
    tile_ids: &[u16; 3],
    tiles: &[TileView],
    fixed_melds: u8,
    visible: &[u8; TILE_KIND_COUNT],
    variant: Variant,
) -> Result<CallCandidate, String> {
    let remaining = remove_tiles(tiles, tile_ids)?;
    let counts = tile_counts(&remaining)?;
    let value = shanten(&counts, fixed_melds + 1);
    let (ukeire, _) = acceptance(&counts, fixed_melds + 1, value, visible, variant);
    Ok(CallCandidate {
        name: "riichi.open_kan",
        label: "明杠",
        tile_ids: tile_ids.to_vec(),
        shanten: value,
        ukeire,
        priority: 2,
    })
}

fn remove_tiles<const N: usize>(
    tiles: &[TileView],
    tile_ids: &[u16; N],
) -> Result<Vec<TileView>, String> {
    let selected = tile_ids.iter().copied().collect::<HashSet<_>>();
    if selected.len() != N
        || !selected
            .iter()
            .all(|id| tiles.iter().any(|tile| tile.id == *id))
    {
        return Err("副露候选牌不在手牌中".to_owned());
    }
    Ok(tiles
        .iter()
        .filter(|tile| !selected.contains(&tile.id))
        .cloned()
        .collect())
}

fn compare_discards(
    left: &(DiscardDecision, u16, bool, usize),
    right: &(DiscardDecision, u16, bool, usize),
) -> Ordering {
    left.0
        .shanten
        .cmp(&right.0.shanten)
        .then_with(|| right.0.ukeire.cmp(&left.0.ukeire))
        .then_with(|| right.0.effective_kinds.cmp(&left.0.effective_kinds))
        .then_with(|| left.1.cmp(&right.1))
        .then_with(|| left.2.cmp(&right.2))
        .then_with(|| right.3.cmp(&left.3))
        .then_with(|| left.0.tile_id.cmp(&right.0.tile_id))
}

fn compare_calls(left: &CallCandidate, right: &CallCandidate) -> Ordering {
    left.shanten
        .cmp(&right.shanten)
        .then_with(|| right.ukeire.cmp(&left.ukeire))
        .then_with(|| right.priority.cmp(&left.priority))
}

#[cfg(test)]
mod tests {
    use super::{TILE_KIND_COUNT, best_discard, shanten, tile_counts, tile_kind};
    use crate::model::TileView;
    use crate::runner::Variant;

    #[test]
    fn recognizes_regular_seven_pairs_and_kokushi_shapes() {
        assert_eq!(shape_shanten("123m123p123s111z55z"), -1);
        assert_eq!(shape_shanten("1122m3344p5566s7z"), 0);
        assert_eq!(shape_shanten("19m19p19s1234567z1m"), -1);
    }

    #[test]
    fn keeps_lowest_shanten_then_largest_acceptance() {
        let tiles = tiles("123m456m789m22p57s1z");
        let mut visible = [0_u8; TILE_KIND_COUNT];
        for tile in &tiles {
            visible[tile_kind(&tile.code).expect("kind")] += 1;
        }
        let decision = best_discard(&tiles, 0, &visible, Variant::Yonma).expect("decision");
        assert_eq!(decision.tile_code, "1z");
        assert_eq!(decision.shanten, 0);
        assert!(decision.ukeire > 0);
    }

    #[test]
    fn sanma_never_counts_removed_man_tiles_as_acceptance() {
        let counts = counts("19m123456789p11z");
        let visible = [0_u8; TILE_KIND_COUNT];
        let (_, yonma_kinds) =
            super::acceptance(&counts, 0, shanten(&counts, 0), &visible, Variant::Yonma);
        let (_, sanma_kinds) =
            super::acceptance(&counts, 0, shanten(&counts, 0), &visible, Variant::Sanma);
        assert!(sanma_kinds <= yonma_kinds);
        for kind in Variant::Sanma.tile_kinds() {
            assert!(!(1..=7).contains(&kind));
        }
    }

    #[test]
    fn red_five_uses_the_normal_five_kind() {
        assert_eq!(tile_kind("0m"), tile_kind("5m"));
        assert_eq!(tile_kind("0p"), tile_kind("5p"));
        assert_eq!(tile_kind("0s"), tile_kind("5s"));
    }

    fn shape_shanten(codes: &str) -> i8 {
        shanten(&counts(codes), 0)
    }

    fn counts(codes: &str) -> [u8; TILE_KIND_COUNT] {
        tile_counts(&tiles(codes)).expect("counts")
    }

    fn tiles(codes: &str) -> Vec<TileView> {
        let mut result = Vec::new();
        let mut digits = Vec::new();
        let mut id = 0_u16;
        for character in codes.chars() {
            if character.is_ascii_digit() {
                digits.push(character);
                continue;
            }
            for digit in digits.drain(..) {
                result.push(TileView {
                    id,
                    code: format!("{digit}{character}"),
                });
                id += 1;
            }
        }
        result
    }
}
