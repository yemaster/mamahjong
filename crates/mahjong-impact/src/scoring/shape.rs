//! 牌型识别。财神在**所有**牌型里都能当任意一张牌。
//!
//! 这里所有函数都只看「非财神的牌种计数 + 财神张数」，副露的信息由调用方折算成
//! `sets_needed` / `pairs_needed` 之类的参数传进来。
//!
//! 一处有意为之的宽松：财神替代时**不检查同一种牌是否超过四张**。规则书没有写这条
//! 限制，而手上最多只有四张财神，硬加限制反而偏离了规则原文。

use crate::tile::TileKind;

pub(crate) const KIND_COUNT: usize = TileKind::COUNT;
const SUITED_KIND_COUNT: usize = 27;
const RANKS_PER_SUIT: usize = 9;

/// 一手牌里非财神部分的牌种计数。
pub(crate) type KindCounts = [u8; KIND_COUNT];

pub(crate) fn total(counts: &KindCounts) -> u8 {
    counts.iter().copied().sum()
}

fn lowest_present(counts: &KindCounts) -> Option<usize> {
    counts.iter().position(|count| *count > 0)
}

const fn is_suited(index: usize) -> bool {
    index < SUITED_KIND_COUNT
}

/// 数牌在本花色内的序数（0 表示一，8 表示九）。
const fn rank_offset(index: usize) -> usize {
    index % RANKS_PER_SUIT
}

/// 标准型：`sets_needed` 组面子 + 可选一对将。
///
/// `runs_allowed == false` 时只认刻子，用来判对对和。冲击麻将不能吃，但暗顺合法。
pub(crate) fn standard(
    counts: &KindCounts,
    jokers: u8,
    sets_needed: u8,
    need_pair: bool,
    runs_allowed: bool,
) -> bool {
    let expected = u16::from(sets_needed) * 3 + if need_pair { 2 } else { 0 };
    if u16::from(total(counts)) + u16::from(jokers) != expected {
        return false;
    }

    let mut working = *counts;
    decompose(&mut working, jokers, sets_needed, need_pair, runs_allowed)
}

fn decompose(
    counts: &mut KindCounts,
    jokers: u8,
    sets_needed: u8,
    need_pair: bool,
    runs_allowed: bool,
) -> bool {
    let Some(index) = lowest_present(counts) else {
        // 只剩财神：正好凑满剩下的面子和将就成立。
        let needed = u16::from(sets_needed) * 3 + if need_pair { 2 } else { 0 };
        return u16::from(jokers) == needed;
    };

    if need_pair {
        if counts[index] >= 2 {
            counts[index] -= 2;
            if decompose(counts, jokers, sets_needed, false, runs_allowed) {
                counts[index] += 2;
                return true;
            }
            counts[index] += 2;
        }
        if jokers >= 1 {
            counts[index] -= 1;
            if decompose(counts, jokers - 1, sets_needed, false, runs_allowed) {
                counts[index] += 1;
                return true;
            }
            counts[index] += 1;
        }
    }

    if sets_needed == 0 {
        return false;
    }

    // 刻子：本牌种出 3 / 2 / 1 张，缺的用财神补。
    for used_jokers in 0..=2_u8 {
        let from_hand = 3 - used_jokers;
        if counts[index] < from_hand || jokers < used_jokers {
            continue;
        }
        counts[index] -= from_hand;
        if decompose(
            counts,
            jokers - used_jokers,
            sets_needed - 1,
            need_pair,
            runs_allowed,
        ) {
            counts[index] += from_hand;
            return true;
        }
        counts[index] += from_hand;
    }

    // 顺子：最小的那张必须是真牌，另外两张可以用财神补。
    if runs_allowed && is_suited(index) && rank_offset(index) <= RANKS_PER_SUIT - 3 {
        for joker_for_second in [false, true] {
            for joker_for_third in [false, true] {
                let used_jokers = u8::from(joker_for_second) + u8::from(joker_for_third);
                if jokers < used_jokers {
                    continue;
                }
                if !joker_for_second && counts[index + 1] == 0 {
                    continue;
                }
                if !joker_for_third && counts[index + 2] == 0 {
                    continue;
                }

                counts[index] -= 1;
                if !joker_for_second {
                    counts[index + 1] -= 1;
                }
                if !joker_for_third {
                    counts[index + 2] -= 1;
                }

                let matched = decompose(
                    counts,
                    jokers - used_jokers,
                    sets_needed - 1,
                    need_pair,
                    runs_allowed,
                );

                counts[index] += 1;
                if !joker_for_second {
                    counts[index + 1] += 1;
                }
                if !joker_for_third {
                    counts[index + 2] += 1;
                }
                if matched {
                    return true;
                }
            }
        }
    }

    false
}

/// 手上最多能凑出几个对子（财神先去补单张，剩下的两两成对）。
pub(crate) fn max_pairs(counts: &KindCounts, jokers: u8) -> u8 {
    let natural: u8 = counts.iter().map(|count| count / 2).sum();
    let singles = u8::try_from(counts.iter().filter(|count| **count % 2 == 1).count())
        .expect("at most 34 kinds");
    let paired_with_jokers = singles.min(jokers);
    natural + paired_with_jokers + (jokers - paired_with_jokers) / 2
}

/// 七对子族：`pairs_needed` 个对子 + `filler` 张任意牌。
///
/// 冲击麻将的「一杠一达，二杠二达」——一个杠算两个对子，同时把手牌少掉的那一张
/// 换成一张任意牌——就是用 `pairs_needed` / `filler` 这两个参数表达的。
pub(crate) fn pairs_shape(counts: &KindCounts, jokers: u8, pairs_needed: u8, filler: u8) -> bool {
    let expected = u16::from(pairs_needed) * 2 + u16::from(filler);
    if u16::from(total(counts)) + u16::from(jokers) != expected {
        return false;
    }
    max_pairs(counts, jokers) >= pairs_needed
}

/// 十三不搭：14 张全不成对，且同花色数牌两两相差大于 2。
///
/// 财神当任意牌，所以只要「剩下的空位够放财神」就成立。
pub(crate) fn thirteen_unrelated(counts: &KindCounts, jokers: u8) -> bool {
    if u16::from(total(counts)) + u16::from(jokers) != 14 {
        return false;
    }
    if counts.iter().any(|count| *count > 1) {
        return false;
    }

    let honors_used = u8::try_from(
        counts[SUITED_KIND_COUNT..]
            .iter()
            .filter(|c| **c == 1)
            .count(),
    )
    .expect("at most seven honors");
    let mut capacity = 7 - honors_used;

    for suit in 0..3 {
        let base = suit * RANKS_PER_SUIT;
        let mut present = [false; RANKS_PER_SUIT];
        for (offset, slot) in present.iter_mut().enumerate() {
            *slot = counts[base + offset] == 1;
        }
        let Some(extra) = suit_spare_capacity(&present) else {
            return false;
        };
        capacity += extra;
    }

    jokers <= capacity
}

/// 一个花色里，在保持「两两相差大于 2」的前提下还能再塞几张牌。
///
/// `None` 表示已经放进去的这些牌本身就违规。
fn suit_spare_capacity(present: &[bool; RANKS_PER_SUIT]) -> Option<u8> {
    let used = u8::try_from(present.iter().filter(|slot| **slot).count()).expect("at most nine");
    let mut best: Option<u8> = None;

    for mask in 0_u16..(1 << RANKS_PER_SUIT) {
        let covers_present = present
            .iter()
            .enumerate()
            .all(|(rank, slot)| !slot || mask & (1 << rank) != 0);
        if !covers_present {
            continue;
        }

        let mut previous: Option<usize> = None;
        let mut size = 0_u8;
        let mut spaced = true;
        for rank in 0..RANKS_PER_SUIT {
            if mask & (1 << rank) == 0 {
                continue;
            }
            if previous.is_some_and(|previous| rank - previous < 3) {
                spaced = false;
                break;
            }
            previous = Some(rank);
            size += 1;
        }
        if spaced {
            best = Some(best.map_or(size, |current: u8| current.max(size)));
        }
    }

    best.map(|size| size - used)
}

/// 七嵌：手牌分成 `groups_needed` 组，每组是同花色相差恰好 2 的两张数牌。
pub(crate) fn seven_gaps(counts: &KindCounts, jokers: u8, groups_needed: u8) -> bool {
    if u16::from(total(counts)) + u16::from(jokers) != u16::from(groups_needed) * 2 {
        return false;
    }
    let mut working = *counts;
    gaps(&mut working, jokers, groups_needed)
}

fn gaps(counts: &mut KindCounts, jokers: u8, groups_needed: u8) -> bool {
    let Some(index) = lowest_present(counts) else {
        return jokers == groups_needed * 2;
    };
    if groups_needed == 0 {
        return false;
    }
    // 字牌进不了任何一组「相差 2 的数牌」。
    if !is_suited(index) {
        return false;
    }

    // 最小的那张只能和本花色 +2 的那张成组，或者用一张财神当搭子。
    if rank_offset(index) <= RANKS_PER_SUIT - 3 && counts[index + 2] > 0 {
        counts[index] -= 1;
        counts[index + 2] -= 1;
        if gaps(counts, jokers, groups_needed - 1) {
            counts[index] += 1;
            counts[index + 2] += 1;
            return true;
        }
        counts[index] += 1;
        counts[index + 2] += 1;
    }

    if jokers >= 1 {
        counts[index] -= 1;
        if gaps(counts, jokers - 1, groups_needed - 1) {
            counts[index] += 1;
            return true;
        }
        counts[index] += 1;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{KindCounts, max_pairs, pairs_shape, seven_gaps, standard, thirteen_unrelated};
    use crate::tile::TileKind;

    fn counts(codes: &[&str]) -> KindCounts {
        let mut counts = [0_u8; super::KIND_COUNT];
        for code in codes {
            let kind: TileKind = code.parse().expect("valid tile code");
            counts[kind.index()] += 1;
        }
        counts
    }

    fn hand(spec: &str) -> KindCounts {
        counts(&spec.split_whitespace().collect::<Vec<_>>())
    }

    #[test]
    fn standard_shape_accepts_runs_and_triplets() {
        let tiles = hand("1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z");

        assert!(standard(&tiles, 0, 4, true, true));
        assert!(!standard(&tiles, 0, 4, true, false), "有顺子就不是对对和");
    }

    #[test]
    fn all_triplet_shape_rejects_runs() {
        let tiles = hand("1m 1m 1m 4p 4p 4p 7s 7s 7s 1z 1z 1z 2z 2z");

        assert!(standard(&tiles, 0, 4, true, false));
    }

    #[test]
    fn jokers_fill_any_missing_tile() {
        // 缺 3m 与将，用两张财神补。
        let tiles = hand("1m 2m 4p 5p 6p 7s 8s 9s 1z 1z 1z 3z");

        assert!(standard(&tiles, 2, 4, true, true));
    }

    #[test]
    fn three_jokers_form_a_triplet_on_their_own() {
        let tiles = hand("1m 2m 3m 4p 5p 6p 7s 8s 9s 2z 2z");

        assert!(standard(&tiles, 3, 4, true, true));
    }

    #[test]
    fn standard_shape_needs_the_exact_tile_count() {
        let tiles = hand("1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z");

        assert!(!standard(&tiles, 0, 4, true, true), "13 张不该判成和牌");
    }

    #[test]
    fn standard_shape_counts_melds_through_sets_needed() {
        // 两组副露 + 手上 2 面子 1 将。
        let tiles = hand("1m 2m 3m 4p 5p 6p 9s 9s");

        assert!(standard(&tiles, 0, 2, true, true));
    }

    #[test]
    fn seven_pairs_counts_a_kan_as_two_pairs() {
        // 零杠：14 张 7 对。
        let plain = hand("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s 1z 1z");
        assert!(pairs_shape(&plain, 0, 7, 0));

        // 一杠：手上 11 张 = 5 对 + 1 张任意牌。
        let one_kan = hand("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 9s");
        assert!(pairs_shape(&one_kan, 0, 5, 1));

        // 二杠：手上 8 张 = 3 对 + 2 张任意牌。
        let two_kans = hand("1m 1m 3m 3m 5p 5p 7p 9s");
        assert!(pairs_shape(&two_kans, 0, 3, 2));
    }

    #[test]
    fn seven_pairs_uses_jokers_for_missing_halves() {
        let tiles = hand("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 9s");

        assert!(pairs_shape(&tiles, 2, 7, 0));
        assert_eq!(max_pairs(&tiles, 2), 7);
    }

    #[test]
    fn seven_pairs_rejects_a_hand_that_cannot_reach_the_pair_count() {
        let tiles = hand("1m 2m 3m 4m 5m 6m 7m 8m 9m 1p 2p 3p 4p 5p");

        assert!(!pairs_shape(&tiles, 0, 7, 0));
    }

    #[test]
    fn thirteen_unrelated_needs_gaps_greater_than_two() {
        let good = hand("1m 4m 7m 1p 4p 7p 1s 4s 7s 1z 2z 3z 4z 5z");
        let too_close = hand("1m 3m 7m 1p 4p 7p 1s 4s 7s 1z 2z 3z 4z 5z");
        let paired = hand("1m 4m 7m 1p 4p 7p 1s 4s 7s 1z 1z 3z 4z 5z");

        assert!(thirteen_unrelated(&good, 0));
        assert!(!thirteen_unrelated(&too_close, 0));
        assert!(!thirteen_unrelated(&paired, 0));
    }

    #[test]
    fn thirteen_unrelated_accepts_jokers_when_slots_remain() {
        // 13 张合法 + 1 张财神：还剩 6z / 7z 之类的空位。
        let tiles = hand("1m 4m 7m 1p 4p 7p 1s 4s 7s 1z 2z 3z 4z");
        assert!(thirteen_unrelated(&tiles, 1));

        // 16 是理论上限（3 花色 ×3 + 7 字牌），塞满之后就没有空位了。
        let full = hand("1m 4m 7m 1p 4p 7p 1s 4s 7s 1z 2z 3z 4z 5z 6z 7z");
        assert!(!thirteen_unrelated(&full, 0), "16 张不是 14 张");
    }

    #[test]
    fn seven_gaps_pairs_tiles_two_ranks_apart() {
        let tiles = hand("1m 3m 2m 4m 5p 7p 6p 8p 1s 3s 4s 6s 5s 7s");

        assert!(seven_gaps(&tiles, 0, 7));
    }

    #[test]
    fn seven_gaps_rejects_honors_and_wrong_gaps() {
        let honors = hand("1m 3m 2m 4m 5p 7p 6p 8p 1s 3s 4s 6s 1z 1z");
        let adjacent = hand("1m 2m 2m 4m 5p 7p 6p 8p 1s 3s 4s 6s 5s 7s");

        assert!(!seven_gaps(&honors, 0, 7));
        assert!(!seven_gaps(&adjacent, 0, 7));
    }

    #[test]
    fn seven_gaps_lets_jokers_stand_in_for_a_partner() {
        let tiles = hand("1m 3m 2m 4m 5p 7p 6p 8p 1s 3s 4s 6s 5s");

        assert!(seven_gaps(&tiles, 1, 7));
    }

    #[test]
    fn two_jokers_form_a_gap_group_on_their_own() {
        let tiles = hand("1m 3m 2m 4m 5p 7p 6p 8p 1s 3s 4s 6s");

        assert!(seven_gaps(&tiles, 2, 7));
    }
}
