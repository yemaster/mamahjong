//! 牌型识别。
//!
//! 四川麻将没有财神，`jokers` 参数一律传 0 保留兼容。这里所有函数都只看「牌种计数」，
//! 副露的信息由调用方折算成 `sets_needed` / `pairs_needed` 之类的参数传进来。

use crate::tile::TileKind;

pub(crate) const KIND_COUNT: usize = TileKind::COUNT;
const SUITED_KIND_COUNT: usize = TileKind::SUITED_KIND_COUNT;
const RANKS_PER_SUIT: usize = 9;

/// 一手牌里非财神部分的牌种计数（四川麻将无财神，即全部手牌）。
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
/// `runs_allowed == false` 时只认刻子，用来判对对和。
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
fn max_pairs(counts: &KindCounts, jokers: u8) -> u8 {
    let natural: u8 = counts.iter().map(|count| count / 2).sum();
    let singles = u8::try_from(counts.iter().filter(|count| **count % 2 == 1).count())
        .expect("at most 34 kinds");
    let paired_with_jokers = singles.min(jokers);
    natural + paired_with_jokers + (jokers - paired_with_jokers) / 2
}

/// 七对子族：`pairs_needed` 个对子 + `filler` 张任意牌。四川麻将七对即 7 对 + 0 张。
pub(crate) fn pairs_shape(counts: &KindCounts, jokers: u8, pairs_needed: u8, filler: u8) -> bool {
    let expected = u16::from(pairs_needed) * 2 + u16::from(filler);
    if u16::from(total(counts)) + u16::from(jokers) != expected {
        return false;
    }
    max_pairs(counts, jokers) >= pairs_needed
}

#[cfg(test)]
mod tests {
    use super::{KindCounts, pairs_shape, standard};
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
        let tiles = hand("1m 2m 3m 4p 5p 6p 7s 8s 9s 1m 1m 1m 2p 2p");

        assert!(standard(&tiles, 0, 4, true, true));
        assert!(!standard(&tiles, 0, 4, true, false), "有顺子就不是对对和");
    }

    #[test]
    fn all_triplet_shape_rejects_runs() {
        let tiles = hand("1m 1m 1m 4p 4p 4p 7s 7s 7s 2m 2m 2m 3p 3p");

        assert!(standard(&tiles, 0, 4, true, false));
    }

    #[test]
    fn standard_shape_needs_the_exact_tile_count() {
        let tiles = hand("1m 2m 3m 4p 5p 6p 7s 8s 9s 1m 1m 1m 2p");

        assert!(!standard(&tiles, 0, 4, true, true), "13 张不该判成和牌");
    }

    #[test]
    fn standard_shape_counts_melds_through_sets_needed() {
        // 两组副露 + 手上 2 面子 1 将。
        let tiles = hand("1m 2m 3m 4p 5p 6p 9s 9s");

        assert!(standard(&tiles, 0, 2, true, true));
    }

    #[test]
    fn seven_pairs_needs_seven_pairs() {
        let plain = hand("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s 6s 6s");
        assert!(pairs_shape(&plain, 0, 7, 0));

        let six = hand("1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s 6s");
        assert!(!pairs_shape(&six, 0, 7, 0));
    }

    #[test]
    fn seven_pairs_accepts_four_identical_as_two_pairs() {
        // 四张一样的牌算两个对子。
        let case = hand("1m 1m 1m 1m 3m 3m 5p 5p 7p 7p 2s 2s 4s 4s");

        assert!(pairs_shape(&case, 0, 7, 0));
    }
}
