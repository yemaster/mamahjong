use std::cmp::Ordering;
use std::collections::HashSet;

use serde_json::{Value, json};

use crate::model::{MatchView, ReactionOptionView, TileView};
use crate::runner::Variant;

const TILE_KIND_COUNT: usize = 34;
const TERMINAL_AND_HONOR_KINDS: [usize; 13] = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BotCommand {
    pub name: &'static str,
    pub payload: Option<Value>,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Riichi strategy
// ---------------------------------------------------------------------------

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
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;

    if view.turn_actions.can_tsumo {
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

    let riichi_tile_ids = &view.turn_actions.riichi_discard_tile_ids;
    if !riichi_tile_ids.is_empty() && decision.shanten == 0 {
        let riichi_decision = if riichi_tile_ids.contains(&decision.tile_id) {
            decision.clone()
        } else {
            best_discard_among(tiles, fixed_melds, &visible, variant, Some(riichi_tile_ids))?
        };
        if riichi_decision.shanten == 0 {
            return Ok(discard_command("riichi.riichi_discard", riichi_decision));
        }
    }

    Ok(discard_command("riichi.discard", decision))
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
            // Impact-only reactions — not reachable from the riichi path, but
            // the match arm has to be exhaustive.
            ReactionOptionView::ImpactPon { .. } | ReactionOptionView::ImpactOpenKan => continue,
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
    best_discard_among(tiles, fixed_melds, visible, variant, None)
}

pub fn best_discard_among(
    tiles: &[TileView],
    fixed_melds: u8,
    visible: &[u8; TILE_KIND_COUNT],
    variant: Variant,
    allowed: Option<&[u16]>,
) -> Result<DiscardDecision, String> {
    if tiles.is_empty() {
        return Err("手牌为空".to_owned());
    }
    let counts = tile_counts(tiles)?;
    let mut candidates = Vec::with_capacity(tiles.len());
    for tile in tiles {
        if allowed.is_some_and(|allowed| !allowed.contains(&tile.id)) {
            continue;
        }
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

// ---------------------------------------------------------------------------
// Impact (冲击麻将) strategy
// ---------------------------------------------------------------------------

/// Generate the best turn action for impact mahjong.
pub fn impact_turn_command(view: &MatchView) -> Result<BotCommand, String> {
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "观察者手牌不可见".to_owned())?;
    let joker_code = view.joker_code();
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;

    // Tsumo always takes priority.
    if view.turn_actions.can_tsumo {
        return Ok(BotCommand {
            name: "impact.tsumo",
            payload: None,
            description: "自摸".to_owned(),
        });
    }

    // Concealed kan (暗杠).  The server sends tile CODES for impact.
    if let Some(ref kan_codes) = view.turn_actions.impact_concealed_kan_tile_codes {
        if let Some(code) = kan_codes.first() {
            return Ok(BotCommand {
                name: "impact.concealed_kan",
                payload: Some(json!({"tile_code": code})),
                description: format!("暗杠 {code}"),
            });
        }
    }

    // Added kan (加杠).
    if let Some(ref meld_ids) = view.turn_actions.impact_added_kan_meld_ids {
        if let Some(meld_id) = meld_ids.first() {
            return Ok(BotCommand {
                name: "impact.added_kan",
                payload: Some(json!({"meld_id": meld_id})),
                description: format!("加杠 {meld_id}"),
            });
        }
    }

    // Indicator concealed kan (指示牌暗杠).
    if view
        .turn_actions
        .impact_indicator_concealed_kan
        .unwrap_or(false)
    {
        return Ok(BotCommand {
            name: "impact.indicator_concealed_kan",
            payload: None,
            description: "指示牌暗杠".to_owned(),
        });
    }

    // Otherwise discard: choose the best tile.
    let visible = visible_counts(view)?;
    let decision = impact_best_discard(tiles, joker_code, fixed_melds, &visible)?;
    Ok(BotCommand {
        name: "impact.discard",
        payload: Some(json!({"tile_id": decision.tile_id})),
        description: format!(
            "打 {}（{}向听，受入{}枚/{}种）",
            decision.tile_code, decision.shanten, decision.ukeire, decision.effective_kinds
        ),
    })
}

/// Generate the best reaction for impact mahjong.
///
/// Impact has no ron and no chi — only pon, open-kan, and pass.
pub fn impact_reaction_command(view: &MatchView) -> Result<Option<BotCommand>, String> {
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "观察者手牌不可见".to_owned())?;
    let joker_code = view.joker_code();

    // Prioritise open-kan over pon.
    for reaction in &view.available_reactions {
        if matches!(reaction, ReactionOptionView::ImpactOpenKan) {
            return Ok(Some(BotCommand {
                name: "impact.open_kan",
                payload: None,
                description: "明杠".to_owned(),
            }));
        }
    }

    // Pon only when it advances shanten.
    let counts = impact_tile_counts(tiles, joker_code)?;
    let (non_joker_counts, hand_jokers) = split_jokers(&counts, joker_code, tiles)?;
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;
    let baseline = impact_shanten(&non_joker_counts, hand_jokers, fixed_melds);

    // Figure out which tile kind was just discarded (the one we're reacting to).
    // The most recent discard across all players has the highest tile ID.
    let discard_kind = last_discard_kind(view);

    for reaction in &view.available_reactions {
        if let ReactionOptionView::ImpactPon { .. } = reaction {
            // Simulate the pon: remove 2 of the called tile from hand, add 1
            // meld.  Without removing tiles the total would exceed 14 and
            // shanten would always return 8 — the pon would never fire.
            if let Some(kind) = discard_kind {
                let mut after_counts = non_joker_counts;
                let remove = after_counts[kind].min(2);
                after_counts[kind] = after_counts[kind].saturating_sub(remove);
                let effective_melds = fixed_melds + 1;
                let after_pon = impact_shanten(&after_counts, hand_jokers, effective_melds);

                if after_pon < baseline {
                    return Ok(Some(BotCommand {
                        name: "impact.pon",
                        payload: None,
                        description: format!(
                            "碰 {}（{}向听 → {}向听）",
                            kind_name(kind),
                            baseline,
                            after_pon
                        ),
                    }));
                }
            }
        }
    }

    Ok(Some(BotCommand {
        name: "impact.pass",
        payload: None,
        description: "过".to_owned(),
    }))
}

/// Impact fallback: when tsumo is rejected, discard the best non-winning tile.
pub fn impact_fallback_discard(view: &MatchView) -> Result<BotCommand, String> {
    let player = view.observer()?;
    let tiles = player
        .concealed_tiles
        .as_deref()
        .ok_or_else(|| "观察者手牌不可见".to_owned())?;
    let joker_code = view.joker_code();
    let fixed_melds = u8::try_from(player.melds.len()).map_err(|_| "副露数量溢出")?;
    let visible = visible_counts(view)?;
    let decision = impact_best_discard(tiles, joker_code, fixed_melds, &visible)?;
    Ok(BotCommand {
        name: "impact.discard",
        payload: Some(json!({"tile_id": decision.tile_id})),
        description: format!(
            "打 {}（{}向听，受入{}枚/{}种）",
            decision.tile_code, decision.shanten, decision.ukeire, decision.effective_kinds
        ),
    })
}

/// Best discard for impact mahjong: joker-aware shanten + acceptance.
pub fn impact_best_discard(
    tiles: &[TileView],
    joker_code: Option<&str>,
    fixed_melds: u8,
    visible: &[u8; TILE_KIND_COUNT],
) -> Result<DiscardDecision, String> {
    if tiles.is_empty() {
        return Err("手牌为空".to_owned());
    }

    let counts = impact_tile_counts(tiles, joker_code)?;
    let (non_joker_counts, hand_jokers) = split_jokers(&counts, joker_code, tiles)?;

    let mut candidates = Vec::with_capacity(tiles.len());
    for tile in tiles {
        // Never discard a joker — they're too valuable.
        if joker_code.is_some_and(|code| tile.code == code) {
            continue;
        }
        let kind = tile_kind(&tile.code)?;
        let mut after = non_joker_counts;
        after[kind] = after[kind]
            .checked_sub(1)
            .ok_or_else(|| "牌张计数不一致".to_owned())?;
        let base_shanten = impact_shanten(&after, hand_jokers, fixed_melds);
        let (ukeire, effective_kinds) =
            impact_acceptance(&after, hand_jokers, fixed_melds, base_shanten, visible);
        candidates.push((
            DiscardDecision {
                tile_id: tile.id,
                tile_code: tile.code.clone(),
                shanten: base_shanten,
                ukeire,
                effective_kinds,
            },
            connection_score(&non_joker_counts, kind),
            is_red(&tile.code),
            kind,
        ));
    }
    if candidates.is_empty() {
        return Err("没有非财神的牌可打".to_owned());
    }
    candidates.sort_by(compare_discards);
    Ok(candidates.into_iter().next().unwrap().0)
}

/// Joker-aware shanten for impact mahjong.
///
/// `counts` is the NON-joker tile counts (34 kinds).
/// `jokers` is the number of joker tiles in hand.
pub fn impact_shanten(counts: &[u8; TILE_KIND_COUNT], jokers: u8, fixed_melds: u8) -> i8 {
    let regular = impact_regular_shanten(counts, jokers, fixed_melds);
    if fixed_melds > 0 {
        return regular;
    }
    regular
        .min(impact_seven_pairs_shanten(counts, jokers))
        .min(thirteen_orphans_shanten_with_jokers(counts, jokers))
}

/// Regular (4 melds + 1 pair) shanten with joker support.
fn impact_regular_shanten(counts: &[u8; TILE_KIND_COUNT], jokers: u8, fixed_melds: u8) -> i8 {
    let total_kinds: u32 = counts.iter().map(|&c| u32::from(c)).sum();
    let total_tiles = total_kinds + u32::from(jokers) + u32::from(fixed_melds) * 3;
    // We must have between 1 and 14 tiles.
    if !(1..=14).contains(&total_tiles) {
        return 8;
    }

    let mut best: i8 = 8;

    // Allocate jokers into pre-formed melds (3 jokers each) and a pre-formed
    // pair (2 jokers), then let the search use any remaining jokers as singles.
    for joker_melds in 0..=jokers / 3 {
        for use_joker_pair in [false, true] {
            let already_used = joker_melds * 3 + if use_joker_pair { 2 } else { 0 };
            if already_used > jokers {
                continue;
            }
            let total_melds = fixed_melds + joker_melds;
            if total_melds > 4 {
                continue;
            }

            let remaining_jokers = jokers - already_used;
            let mut mutable = *counts;
            regular_search_impact(
                &mut mutable,
                0,
                total_melds,
                use_joker_pair,
                0,
                remaining_jokers,
                &mut best,
            );

            if best <= 0 {
                return best;
            }
        }
    }

    best
}

/// Recursive search for the standard shape, with jokers as a single-tile
/// resource pool.  Mirrors `regular_search` but each joker can fill one missing
/// tile in a meld, a pair, or an incomplete set.
fn regular_search_impact(
    counts: &mut [u8; TILE_KIND_COUNT],
    mut index: usize,
    melds: u8,
    has_pair: bool,
    incomplete: u8,
    jokers: u8,
    best: &mut i8,
) {
    while index < TILE_KIND_COUNT && counts[index] == 0 {
        index += 1;
    }

    if index == TILE_KIND_COUNT {
        // All non-joker kinds are processed.  What can the remaining jokers do?
        let incomplete = incomplete.min(4_u8.saturating_sub(melds));
        let missing_pairs = u8::from(!has_pair);

        // One joker can fill one incomplete slot.
        let filled_by_jokers = jokers.min(incomplete);
        let jokers_after = jokers - filled_by_jokers;
        let adjusted_incomplete = incomplete - filled_by_jokers;

        // Two remaining jokers can form the missing pair.
        let pair_from_jokers = if missing_pairs > 0 && jokers_after >= 2 {
            1
        } else {
            0
        };
        let has_pair_after = has_pair || pair_from_jokers > 0;
        let jokers_after_pair = jokers_after - pair_from_jokers * 2;

        // Three remaining jokers can form a full meld, but never exceed 4 total.
        let meld_capacity = 4_u8.saturating_sub(melds);
        let melds_from_jokers = (jokers_after_pair / 3).min(meld_capacity);
        let jokers_after_melds = jokers_after_pair - melds_from_jokers * 3;

        // Leftover jokers fill one more incomplete slot each.
        let incomplete_capacity = 4_u8.saturating_sub(melds + melds_from_jokers);
        let extra_fill = jokers_after_melds.min(adjusted_incomplete.min(incomplete_capacity));
        let final_incomplete = adjusted_incomplete - extra_fill;

        let total_melds = melds + melds_from_jokers;
        let value = 8 - i8::try_from(
            total_melds * 2
                + (4_u8.saturating_sub(total_melds)).min(final_incomplete)
                + u8::from(has_pair_after),
        )
        .expect("small counts");
        *best = (*best).min(value);
        return;
    }

    let unused = counts[index];
    counts[index] = 0;

    // Skip this kind entirely.
    regular_search_impact(counts, index + 1, melds, has_pair, incomplete, jokers, best);
    counts[index] = unused;

    // Try forming a triplet: use up to 2 jokers to fill missing tiles.
    for use_jokers in 0..=2_u8 {
        let needed = 3_u8.saturating_sub(unused);
        if use_jokers < needed || use_jokers > jokers {
            continue;
        }
        let from_hand = 3 - use_jokers;
        if unused < from_hand {
            continue;
        }
        if melds >= 4 {
            continue;
        }
        counts[index] -= from_hand;
        regular_search_impact(
            counts,
            index,
            melds + 1,
            has_pair,
            incomplete,
            jokers - use_jokers,
            best,
        );
        counts[index] += from_hand;
    }

    // Try forming a sequence (suited tiles only).
    if index < 27 && index % 9 <= 6 {
        for j2 in [false, true] {
            for j3 in [false, true] {
                let used = u8::from(j2) + u8::from(j3);
                if used > jokers {
                    continue;
                }
                if !j2 && counts[index + 1] == 0 {
                    continue;
                }
                if !j3 && counts[index + 2] == 0 {
                    continue;
                }
                if melds >= 4 {
                    continue;
                }
                counts[index] -= 1;
                if !j2 {
                    counts[index + 1] -= 1;
                }
                if !j3 {
                    counts[index + 2] -= 1;
                }
                regular_search_impact(
                    counts,
                    index,
                    melds + 1,
                    has_pair,
                    incomplete,
                    jokers - used,
                    best,
                );
                counts[index] += 1;
                if !j2 {
                    counts[index + 1] += 1;
                }
                if !j3 {
                    counts[index + 2] += 1;
                }
            }
        }
    }

    // Try forming a pair (need 1 joker if only 1 real tile).
    if unused >= 2 {
        counts[index] -= 2;
        if !has_pair {
            regular_search_impact(counts, index, melds, true, incomplete, jokers, best);
        }
        if incomplete < 4 {
            regular_search_impact(counts, index, melds, has_pair, incomplete + 1, jokers, best);
        }
        counts[index] += 2;
    } else if unused == 1 && jokers >= 1 {
        // One real tile + one joker = pair
        counts[index] -= 1;
        if !has_pair {
            regular_search_impact(counts, index, melds, true, incomplete, jokers - 1, best);
        }
        if incomplete < 4 {
            regular_search_impact(
                counts,
                index,
                melds,
                has_pair,
                incomplete + 1,
                jokers - 1,
                best,
            );
        }
        counts[index] += 1;
    }

    // Incomplete sets: 1 tile + 1 joker (partial sequence, single gap).
    // Only try when the adjacent tile isn't present (the real-tile path
    // handles that case more efficiently).
    if incomplete < 4 && index < 27 && index % 9 <= 7 && jokers >= 1 && counts[index + 1] == 0 {
        counts[index] -= 1;
        regular_search_impact(
            counts,
            index,
            melds,
            has_pair,
            incomplete + 1,
            jokers - 1,
            best,
        );
        counts[index] += 1;
    }

    // Incomplete set: index and index+2 present, forming a 2-tile partial
    // sequence.  (When `counts[index+2] == 0`, the lone-tile-incomplete path
    // below already covers the case.)
    if incomplete < 4 && jokers >= 1 && index < 27 && index % 9 <= 6 && counts[index + 2] > 0 {
        counts[index] -= 1;
        regular_search_impact(
            counts,
            index,
            melds,
            has_pair,
            incomplete + 1,
            jokers - 1,
            best,
        );
        counts[index] += 1;
    }

    // Lone tile as an incomplete slot.
    if incomplete < 4 {
        counts[index] -= 1;
        regular_search_impact(counts, index, melds, has_pair, incomplete + 1, jokers, best);
        counts[index] += 1;
    }
}

/// Seven-pairs shanten with jokers.
///
/// Each joker can pair with a lone real tile (eating 1 incomplete), or two
/// jokers can form a standalone pair.
fn impact_seven_pairs_shanten(counts: &[u8; TILE_KIND_COUNT], jokers: u8) -> i8 {
    let natural_pairs: u8 = counts.iter().map(|c| c / 2).sum();
    let singles: u8 =
        u8::try_from(counts.iter().filter(|c| *c % 2 == 1).count()).expect("at most 34");

    // Jokers first pair up singles (1 joker + 1 single = 1 pair).
    let paired_singles = jokers.min(singles);
    let jokers_after = jokers - paired_singles;

    // Remaining jokers form standalone pairs (2 jokers = 1 pair).
    let joker_pairs = jokers_after / 2;

    let total_pairs = natural_pairs + paired_singles + joker_pairs;
    6 - i8::try_from(total_pairs.min(7)).expect("small numbers")
}

/// Thirteen-orphans shanten with jokers.
///
/// Jokers fill empty terminal/honor slots.
fn thirteen_orphans_shanten_with_jokers(counts: &[u8; TILE_KIND_COUNT], jokers: u8) -> i8 {
    let unique: u8 = TERMINAL_AND_HONOR_KINDS
        .iter()
        .filter(|kind| counts[**kind] > 0)
        .count()
        .try_into()
        .expect("at most 13");
    let has_pair = TERMINAL_AND_HONOR_KINDS
        .iter()
        .any(|kind| counts[*kind] >= 2);

    // Jokers fill missing unique slots.
    let missing = 13_u8.saturating_sub(unique);
    let filled_by_jokers = jokers.min(missing);
    let jokers_after = jokers - filled_by_jokers;
    let effective_unique = unique + filled_by_jokers;

    // Remaining jokers can form the pair if needed.
    let effective_pair = has_pair || jokers_after >= 1;

    13 - i8::try_from(effective_unique).expect("at most 13") - i8::from(effective_pair)
}

/// Joker-aware acceptance: count tiles that, if drawn, reduce shanten by ≥1.
fn impact_acceptance(
    counts: &[u8; TILE_KIND_COUNT],
    jokers: u8,
    fixed_melds: u8,
    base_shanten: i8,
    visible: &[u8; TILE_KIND_COUNT],
) -> (u16, u8) {
    let mut total = 0_u16;
    let mut kinds = 0_u8;
    for kind in 0..TILE_KIND_COUNT {
        if counts[kind] >= 4 {
            continue;
        }
        let mut improved = *counts;
        improved[kind] += 1;
        if impact_shanten(&improved, jokers, fixed_melds) < base_shanten {
            let remaining = 4_u8.saturating_sub(visible[kind]);
            if remaining > 0 {
                total += u16::from(remaining);
                kinds += 1;
            }
        }
    }
    (total, kinds)
}

/// Count tiles, identifying jokers separately.
fn impact_tile_counts(
    tiles: &[TileView],
    joker_code: Option<&str>,
) -> Result<[u8; TILE_KIND_COUNT], String> {
    let mut counts = [0_u8; TILE_KIND_COUNT];
    for tile in tiles {
        if joker_code.is_some_and(|code| tile.code == code) {
            // Jokers do not contribute to any tile kind — they are counted
            // separately via `split_jokers`.
            continue;
        }
        let kind = tile_kind(&tile.code)?;
        counts[kind] = counts[kind]
            .checked_add(1)
            .ok_or_else(|| "牌张计数溢出".to_owned())?;
    }
    Ok(counts)
}

/// Returns `(non_joker_counts, joker_count)`.
fn split_jokers(
    counts: &[u8; TILE_KIND_COUNT],
    joker_code: Option<&str>,
    tiles: &[TileView],
) -> Result<([u8; TILE_KIND_COUNT], u8), String> {
    if joker_code.is_none() {
        return Ok((*counts, 0));
    }
    let joker_count = u8::try_from(
        tiles
            .iter()
            .filter(|tile| joker_code.is_some_and(|code| tile.code == code))
            .count(),
    )
    .map_err(|_| "财神数量溢出".to_owned())?;
    Ok((*counts, joker_count))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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
    if let Some(ref tile) = view.joker_indicator {
        add(tile)?;
    }
    for player in &view.players {
        if let Some(tiles) = &player.concealed_tiles {
            for tile in tiles {
                // Jokers are wildcards — they don't consume any single kind's
                // visibility, because they can become any tile.
                if view.joker_code().is_some_and(|code| tile.code == code) {
                    continue;
                }
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

/// Find the most recently discarded tile kind across all players.
///
/// During the reaction phase the trigger seat just discarded a tile; the
/// globally highest tile ID in any discard pile identifies it.
fn last_discard_kind(view: &MatchView) -> Option<usize> {
    view.players
        .iter()
        .flat_map(|player| player.discards.iter().map(move |discard| &discard.tile))
        .max_by_key(|tile| tile.id)
        .and_then(|tile| tile_kind(&tile.code).ok())
}

/// Human-readable tile kind name for logging.
fn kind_name(kind: usize) -> String {
    if kind >= 34 {
        return "?".to_owned();
    }
    if kind >= 27 {
        return format!("{}z", kind - 26);
    }
    let suit = ["m", "p", "s"][kind / 9];
    let rank = (kind % 9) + 1;
    format!("{rank}{suit}")
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
    use super::{
        TILE_KIND_COUNT, best_discard, impact_seven_pairs_shanten, impact_shanten, shanten,
        thirteen_orphans_shanten_with_jokers, tile_counts, tile_kind,
    };
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

    // -- impact shanten tests --

    #[test]
    fn joker_reduces_shanten_by_one() {
        // Adding jokers should never increase shanten (monotonicity).
        let c1 = counts("123m456p789s1z");
        let base = impact_shanten(&c1, 0, 0);
        assert!(impact_shanten(&c1, 1, 0) <= base);
        assert!(impact_shanten(&c1, 2, 0) <= base);

        // With enough jokers a hand eventually becomes winning.
        // 4 jokers + 3 melds + 1 lone → 4th meld (3 jokers) + pair (1 joker) = -1.
        assert_eq!(impact_shanten(&c1, 4, 0), -1);

        // 3 jokers form a complete meld → reduces shanten significantly.
        // 111m 222p 44z 35s: 2 melds + 1 pair + 2 incompletes → with 3 jokers
        // forming one meld, we get shanten lower.
        let c2 = counts("111m222p44z35s");
        let without = impact_shanten(&c2, 0, 0);
        let with_three = impact_shanten(&c2, 3, 0);
        assert!(
            with_three < without,
            "3 jokers should reduce shanten: with={with_three}, without={without}"
        );
    }

    #[test]
    fn joker_completes_a_triplet() {
        // 11m 22p 33s 44z 55m + 3 jokers — should be very close to tenpai
        let counts = counts("11m22p33s44z55m");
        // With 3 jokers as a triplet, should be close to winning
        let shanten = impact_shanten(&counts, 3, 0);
        assert!(
            shanten <= 1,
            "3 jokers should get close to tenpai, got {shanten}"
        );
    }

    #[test]
    fn four_jokers_form_a_winning_hand() {
        // 3 melds + 1 lone + 4 jokers.
        // 3 jokers form the 4th meld, 1 joker pairs with the lone → -1.
        let counts = counts("123m456p789s1z"); // 10 tiles: 3 melds + 1 lone → 1-shanten
        assert_eq!(impact_shanten(&counts, 0, 0), 1);
        assert_eq!(impact_shanten(&counts, 4, 0), -1);
    }

    #[test]
    fn jokers_help_seven_pairs() {
        // 5 natural pairs (10 tiles) + 2 singles + 2 jokers = 14 tiles.
        // Jokers pair with the 2 singles → 7 pairs = winning (-1).
        let counts = counts("11m22p33s44z55m1p3s"); // 12 tiles: 5 pairs + 2 singles
        assert_eq!(impact_seven_pairs_shanten(&counts, 0), 1); // 5 pairs → 6−5 = 1-shanten
        assert_eq!(impact_seven_pairs_shanten(&counts, 2), -1);
    }

    #[test]
    fn jokers_help_thirteen_orphans() {
        let counts = counts("1m9m1p9p1s9s1z2z3z4z5z6z");
        // 12 out of 13, 1 joker fills the last, but still need a pair
        assert!(thirteen_orphans_shanten_with_jokers(&counts, 2) <= 1);
    }

    #[test]
    fn impact_shanten_handles_fixed_melds() {
        // Hand with 1 fixed meld already
        let counts = counts("123m11z"); // 2 tiles left + 1 meld = 5 tiles
        let shanten = impact_shanten(&counts, 0, 1);
        assert!(shanten >= 0);
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
