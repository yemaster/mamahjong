//! 牌山：骰子决定开门位、留墩数与财神指示牌。
//!
//! 冲击麻将没有王牌区。整座牌山里只有翻财神那一墩不能摸——翻开的指示牌和它下面
//! 那张一起留在山上，也就是 134 张可摸牌。杠张从摸牌序列的**末尾**取，不额外补花。
//!
//! 摸牌逻辑与立直麻将完全一致：一副洗好的平铺牌，从下标 0 顺序摸到尾。
//! 没有墙序、没有顺时针 / 逆时针——「方向」在这一层根本不存在。
//!
//! 和立直麻将唯一的两处区别：
//!
//! | | 立直麻将 | 冲击麻将 |
//! |---|---|---|
//! | 不可摸的牌 | 末尾 14 张王牌 | 翻财神那一墩 2 张 |
//! | 指示牌位置 | 倒数第 6 张（`live_end + 8`） | 倒数第 `2*(x+x+y)-1` 张 |
//!
//! 其中 `x` 是较小那颗骰子、`x+y` 是点数和。指示牌正下方那张
//! （倒数第 `2*(x+x+y)`）也不能摸。这两张之外的 134 张全部可摸，
//! **包括指示牌后面那一段**——那不是王牌区，是牌山的末尾。
//!
//! 骰子的另外两个用途（割目家、预留墩数）只是物理摆放的说法：
//! 牌本来就是现洗的，从哪儿断开对随机性没有任何影响，所以摸牌顺序
//! 不需要按墙遍历。`x` 真正起作用的地方只有上面那个指示牌偏移量。

use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

use crate::progress::Seat;
use crate::tile::{Tile, TileId, TileKind, full_tile_set, joker_of};

const WALL_SEED_SIZE: usize = 32;

/// 牌山的面数，和座位数一致。
pub const WALL_COUNT: usize = 4;
/// 每面墙的墩数。
pub const STACKS_PER_WALL: usize = 17;
/// 每墩的张数（上层 + 下层）。
pub const TILES_PER_STACK: usize = 2;
/// 一座牌山的总张数。
pub const WALL_TILE_COUNT: usize = WALL_COUNT * STACKS_PER_WALL * TILES_PER_STACK;

#[derive(Clone, Eq, PartialEq)]
pub struct WallSeed([u8; WALL_SEED_SIZE]);

impl WallSeed {
    pub fn generate() -> Result<Self, SeedGenerationError> {
        let mut bytes = [0_u8; WALL_SEED_SIZE];
        getrandom::fill(&mut bytes).map_err(SeedGenerationError)?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; WALL_SEED_SIZE]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn expose_bytes(&self) -> &[u8; WALL_SEED_SIZE] {
        &self.0
    }
}

impl Debug for WallSeed {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("WallSeed([REDACTED])")
    }
}

#[derive(Debug)]
pub struct SeedGenerationError(getrandom::Error);

impl Display for SeedGenerationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to obtain operating-system randomness: {}",
            self.0
        )
    }
}

impl Error for SeedGenerationError {}

/// 两颗骰子。点数与和值决定开门位、留墩数与财神指示牌的位置。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dice {
    first: u8,
    second: u8,
}

impl Dice {
    #[must_use]
    pub const fn new(first: u8, second: u8) -> Self {
        debug_assert!(first >= 1 && first <= 6);
        debug_assert!(second >= 1 && second <= 6);
        Self { first, second }
    }

    #[must_use]
    pub const fn first(self) -> u8 {
        self.first
    }

    #[must_use]
    pub const fn second(self) -> u8 {
        self.second
    }

    /// 两颗骰子点数之和，2..=12。
    #[must_use]
    pub const fn sum(self) -> u8 {
        self.first + self.second
    }

    /// 较小的一颗，也就是开门那面从左起要留的墩数（物理布局概念）。
    #[must_use]
    pub const fn smaller(self) -> u8 {
        if self.first <= self.second {
            self.first
        } else {
            self.second
        }
    }
}

/// 指示牌是倒数第几张：`2*(x+x+y)-1`，x 是较小那颗骰子、x+y 是点数和。
///
/// 取值范围 5..=35（骰子 1,1 到 6,6），远小于 136，永远落在牌山里。
/// 立直麻将这里是定值 6，冲击麻将改成由骰子算——这就是两者唯一的位置差别。
const fn indicator_offset_from_end(dice: Dice) -> usize {
    2 * (dice.smaller() as usize + dice.sum() as usize) - 1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wall {
    /// 按物理位置排好的 136 张牌，摸牌不会改动它。
    tiles: Box<[Tile]>,
    /// 可摸位置在 `tiles` 中的下标，按摸牌先后排；翻财神那一墩已被剔除。
    order: Box<[usize]>,
    front: usize,
    back: usize,
    dice: Dice,
    break_seat: Seat,
    indicator_position: usize,
}

impl Wall {
    /// 洗牌、掷骰、定开门位与财神指示牌。
    ///
    /// 骰子也由同一颗种子推出，重演时能完全复现。
    #[must_use]
    pub fn new(dealer: Seat, seed: &WallSeed) -> Self {
        let mut tiles = full_tile_set();
        let mut random = ChaCha20Rng::from_seed(seed.0);
        fisher_yates_shuffle(&mut tiles, &mut random);

        let dice = Dice::new(roll_die(&mut random), roll_die(&mut random));
        Self::with_dice(dealer, tiles, dice)
    }

    /// 指定骰子点数铺牌，测试用。
    #[must_use]
    pub(crate) fn with_dice(dealer: Seat, tiles: Vec<Tile>, dice: Dice) -> Self {
        debug_assert_eq!(tiles.len(), WALL_TILE_COUNT);

        // 从庄家数 1、逆时针数到点数和，就是开门那一面（割目家）。只用于展示。
        let break_seat = dealer.offset_by(dice.sum() - 1);

        // 指示牌在倒数第 2*(x+x+y)-1 张，正下方那张（倒数第 2*(x+x+y)）也不能摸。
        // 立直麻将是「倒数第 6 张」这个定值，这里换成由骰子算出来的偏移，
        // 除此之外索引方式一模一样：都是从牌山末尾往回数。
        let indicator_position = WALL_TILE_COUNT - indicator_offset_from_end(dice);
        // 倒数第 k 张的下标是 len - k，所以「下面那张」比指示牌小 1。
        let below_position = indicator_position - 1;

        // 摸牌顺序：和立直麻将一样，从下标 0 顺序摸到尾，只跳过上面那两张。
        // 指示牌后面那一段照样能摸——冲击麻将没有王牌区。
        let mut order = Vec::with_capacity(WALL_TILE_COUNT - TILES_PER_STACK);
        for slot in 0..WALL_TILE_COUNT {
            if slot == indicator_position || slot == below_position {
                continue;
            }
            order.push(slot);
        }

        let back = order.len();
        Self {
            tiles: tiles.into_boxed_slice(),
            order: order.into_boxed_slice(),
            front: 0,
            back,
            dice,
            break_seat,
            indicator_position,
        }
    }

    #[must_use]
    pub const fn dice(&self) -> Dice {
        self.dice
    }

    /// 开门位：割目家（骰子和数到的座位）。
    #[must_use]
    pub const fn break_seat(&self) -> Seat {
        self.break_seat
    }

    /// 财神指示牌。
    #[must_use]
    pub fn indicator(&self) -> Tile {
        self.tiles[self.indicator_position]
    }

    /// 财神：指示牌的下一张。
    #[must_use]
    pub fn joker(&self) -> TileKind {
        joker_of(self.indicator().kind())
    }

    /// 牌山还剩几张可摸（杠张会一并扣掉）。
    #[must_use]
    pub const fn remaining_draws(&self) -> usize {
        self.back.saturating_sub(self.front)
    }

    /// 已经从牌山末端摸走的岭上牌数量。
    ///
    /// `back` 只会被 [`Self::draw_from_back`] 推进，因此它和普通摸牌游标 `front`
    /// 相互独立，正好可以作为客户端判断岭上摸牌是否真正发生的稳定序号。
    #[must_use]
    pub const fn completed_rinshan_draws(&self) -> usize {
        self.order.len() - self.back
    }

    /// 正常摸牌：从摸牌序列的头部取。
    pub fn draw(&mut self) -> Option<Tile> {
        if self.remaining_draws() == 0 {
            return None;
        }
        let tile = self.tiles[self.order[self.front]];
        self.front += 1;
        Some(tile)
    }

    /// 杠张：从摸牌序列的**末尾**取，但一墩之内仍旧先上层后下层。
    ///
    /// 摸牌序列里一墩是「先上层后下层」相邻的两张，直接从尾巴上弹就成了先摸底下
    /// 那张。所以每摸到新的一墩就把这一墩的两张对调一下再弹：弹出来的是上层，剩下
    /// 的下层留在原地，下一个杠（或者正常摸牌摸到这儿）再拿。对调只是换了个位置，
    /// 牌一张不多一张不少，`front`/`back` 那套账照旧。
    pub fn draw_from_back(&mut self) -> Result<Tile, WallError> {
        if self.remaining_draws() == 0 {
            return Err(WallError::Exhausted);
        }
        // 已经取走的杠张数；偶数说明这一杠要开新的一墩。
        let taken = self.order.len() - self.back;
        if taken % 2 == 0 && self.remaining_draws() >= 2 {
            self.order.swap(self.back - 2, self.back - 1);
        }
        self.back -= 1;
        Ok(self.tiles[self.order[self.back]])
    }

    #[must_use]
    pub fn tile_by_id(&self, tile_id: TileId) -> Option<Tile> {
        self.tiles.iter().copied().find(|tile| tile.id() == tile_id)
    }

    /// 洗好之后整座牌山的物理顺序。
    ///
    /// 这是牌谱要用的东西：一局打完之后存下来，重演时才画得出没人摸到的牌。
    /// **对局还没结束就不许发给任何客户端**，那等于直接送一份作弊器出去。
    #[must_use]
    pub fn ordered_tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// 翻开的财神指示牌在 `ordered_tiles()` 里的下标。
    #[must_use]
    pub const fn indicator_position(&self) -> usize {
        self.indicator_position
    }

    /// 摸不到的那两张，按下标升序：指示牌下面那张，和翻开的指示牌。
    ///
    /// 指示牌在倒数第 `2*(x+x+y)-1` 张、下面那张在倒数第 `2*(x+x+y)`，
    /// 「倒数第 k 张」的下标是 `len - k`，所以下面那张的下标反而小 1。
    #[must_use]
    pub const fn dead_positions(&self) -> [usize; TILES_PER_STACK] {
        [self.indicator_position - 1, self.indicator_position]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WallError {
    Exhausted,
}

impl Display for WallError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Exhausted => "the wall has no drawable tiles left",
        };
        formatter.write_str(message)
    }
}

impl Error for WallError {}

fn roll_die(random: &mut impl RngCore) -> u8 {
    u8::try_from(uniform_below(random, 6)).expect("a die roll fits into u8") + 1
}

fn fisher_yates_shuffle<T>(values: &mut [T], random: &mut impl RngCore) {
    for upper_index in (1..values.len()).rev() {
        let bound = u64::try_from(upper_index + 1).expect("slice length fits into u64");
        let selected = usize::try_from(uniform_below(random, bound))
            .expect("sampled index is bounded by the slice length");
        values.swap(upper_index, selected);
    }
}

fn uniform_below(random: &mut impl RngCore, bound: u64) -> u64 {
    debug_assert!(bound > 0);
    let rejection_threshold = bound.wrapping_neg() % bound;
    loop {
        let value = random.next_u64();
        if value >= rejection_threshold {
            return value % bound;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Dice, TILES_PER_STACK, WALL_TILE_COUNT, Wall, WallError, WallSeed};
    use crate::progress::Seat;
    use crate::tile::{full_tile_set, joker_of};

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    fn fixed_seed(value: u8) -> WallSeed {
        WallSeed::from_bytes([value; 32])
    }

    /// 未洗过的一副牌，方便按位置反推是哪张。
    fn ordered_wall(dealer: Seat, dice: Dice) -> Wall {
        Wall::with_dice(dealer, full_tile_set(), dice)
    }

    #[test]
    fn seed_debug_output_never_reveals_secret_bytes() {
        let output = format!("{:?}", fixed_seed(0xab));

        assert_eq!(output, "WallSeed([REDACTED])");
        assert!(!output.contains("ab"));
    }

    #[test]
    fn same_seed_reproduces_identical_wall_and_dice() {
        let seed = fixed_seed(7);
        let first = Wall::new(seat(0), &seed);
        let second = Wall::new(seat(0), &seed);

        assert_eq!(first.ordered_tiles(), second.ordered_tiles());
        assert_eq!(first.dice(), second.dice());
    }

    #[test]
    fn dice_stay_within_one_to_six() {
        for value in 0..32_u8 {
            let wall = Wall::new(seat(0), &fixed_seed(value));
            let dice = wall.dice();

            assert!((1..=6).contains(&dice.first()));
            assert!((1..=6).contains(&dice.second()));
            assert_eq!(dice.smaller(), dice.first().min(dice.second()));
        }
    }

    #[test]
    fn break_seat_counts_the_dealer_as_one_counter_clockwise() {
        // 庄家 = 0 号座位。点数和 1 是庄家自己（不可能出现），2 是下家，以此类推。
        assert_eq!(ordered_wall(seat(0), Dice::new(1, 1)).break_seat(), seat(1));
        assert_eq!(ordered_wall(seat(0), Dice::new(2, 3)).break_seat(), seat(0));
        assert_eq!(ordered_wall(seat(2), Dice::new(3, 4)).break_seat(), seat(0));
        assert_eq!(ordered_wall(seat(3), Dice::new(6, 6)).break_seat(), seat(2));
    }

    #[test]
    fn the_indicator_is_the_nth_tile_counted_back_from_the_end() {
        // 指示牌 = 倒数第 2*(x+x+y)-1 张，下面那张 = 倒数第 2*(x+x+y) 张。
        for first in 1..=6_u8 {
            for second in 1..=6_u8 {
                let dice = Dice::new(first, second);
                let wall = ordered_wall(seat(0), dice);

                let smaller = usize::from(dice.smaller());
                let sum = usize::from(dice.sum());
                let indicator_from_end = 2 * (smaller + sum) - 1;
                let below_from_end = 2 * (smaller + sum);

                // 「倒数第 k 张」的下标是 len - k。
                let indicator_index = WALL_TILE_COUNT - indicator_from_end;
                let below_index = WALL_TILE_COUNT - below_from_end;

                assert_eq!(
                    wall.indicator_position(),
                    indicator_index,
                    "骰子 {first},{second}：指示牌应当在倒数第 {indicator_from_end} 张"
                );
                assert_eq!(
                    wall.indicator(),
                    wall.ordered_tiles()[indicator_index],
                    "指示牌取的是那个下标上的牌"
                );
                assert_eq!(
                    wall.dead_positions(),
                    [below_index, indicator_index],
                    "不能摸的是指示牌和它正下方那张"
                );
                assert_eq!(wall.joker(), joker_of(wall.indicator().kind()));
            }
        }
    }

    #[test]
    fn the_five_five_case_matches_the_hand_worked_example() {
        // x=5, y=5 → 2*(5+10)-1 = 29，指示牌在倒数第 29 张 = 下标 136-29 = 107。
        let wall = ordered_wall(seat(0), Dice::new(5, 5));

        assert_eq!(wall.indicator_position(), 107);
        assert_eq!(wall.dead_positions(), [106, 107]);
    }

    #[test]
    fn draw_order_is_plain_sequential_like_riichi() {
        // 摸牌顺序就是 0,1,2,... 顺序摸到尾，只跳过那两张。
        // 没有墙序、没有方向——和立直麻将逐字一致。
        let dice = Dice::new(2, 5);
        let mut wall = ordered_wall(seat(0), dice);
        let dead = wall.dead_positions();

        let expected_order: Vec<usize> = (0..WALL_TILE_COUNT)
            .filter(|index| !dead.contains(index))
            .collect();
        assert_eq!(
            wall.order.to_vec(),
            expected_order,
            "摸牌顺序必须是纯粹的顺序排列"
        );

        // 头三张就是牌山最前面三张，没有任何重排。
        let drawn: Vec<_> = (0..3)
            .map(|_| wall.draw().expect("wall is not empty"))
            .collect();
        let expected: Vec<_> = (0..3).map(|index| wall.ordered_tiles()[index]).collect();
        assert_eq!(drawn, expected, "开头三张就是下标 0、1、2");
    }

    #[test]
    fn tiles_after_the_indicator_are_still_drawable() {
        // 冲击麻将没有王牌区：指示牌后面那一段照样要摸完。
        let mut wall = Wall::new(seat(0), &fixed_seed(21));
        let indicator_index = wall.indicator_position();
        let tail_ids: Vec<_> = wall.ordered_tiles()[indicator_index + 1..]
            .iter()
            .map(|tile| tile.id())
            .collect();
        assert!(!tail_ids.is_empty(), "指示牌后面确实还有牌");

        let mut drawn = HashSet::new();
        while let Some(tile) = wall.draw() {
            drawn.insert(tile.id());
        }

        for tile_id in tail_ids {
            assert!(drawn.contains(&tile_id), "指示牌后面那一段必须能摸到");
        }
    }

    #[test]
    fn order_contains_no_duplicates_and_excludes_only_dead_tiles() {
        // 对所有骰子组合验证：order 没有重复，不包含死牌，覆盖其余全部。
        for sum_first in 1..=6_u8 {
            for sum_second in 1..=6_u8 {
                let wall = ordered_wall(seat(0), Dice::new(sum_first, sum_second));
                let mut positions: Vec<usize> = wall.order.to_vec();
                let count = positions.len();
                positions.sort_unstable();
                positions.dedup();

                assert_eq!(count, WALL_TILE_COUNT - TILES_PER_STACK);
                assert_eq!(positions.len(), count, "摸牌序列里有重复位置");
                for dead in wall.dead_positions() {
                    assert!(!positions.contains(&dead));
                }
            }
        }
    }

    #[test]
    fn every_tile_but_the_indicator_stack_is_drawable() {
        let mut wall = Wall::new(seat(1), &fixed_seed(11));
        let dead: Vec<_> = wall
            .dead_positions()
            .into_iter()
            .map(|position| wall.ordered_tiles()[position].id())
            .collect();
        let mut drawn = HashSet::new();

        assert_eq!(wall.remaining_draws(), WALL_TILE_COUNT - TILES_PER_STACK);
        while let Some(tile) = wall.draw() {
            assert!(drawn.insert(tile.id()), "同一张牌被摸了两次");
        }

        assert_eq!(drawn.len(), WALL_TILE_COUNT - TILES_PER_STACK);
        for tile_id in dead {
            assert!(!drawn.contains(&tile_id), "翻财神那一墩不应当能摸到");
        }
        assert_eq!(wall.remaining_draws(), 0);
        assert_eq!(wall.draw(), None);
    }

    #[test]
    fn kan_tiles_take_the_upper_tile_of_the_last_stack_first() {
        let mut wall = Wall::new(seat(0), &fixed_seed(12));
        let total = wall.remaining_draws();
        // 摸牌序列的最后一墩：倒数第二张是上层，最后一张是下层。
        let (upper, lower) = {
            let mut probe = Wall::new(seat(0), &fixed_seed(12));
            let mut tiles = Vec::new();
            while let Some(tile) = probe.draw() {
                tiles.push(tile);
            }
            (tiles[tiles.len() - 2], tiles[tiles.len() - 1])
        };

        let first = wall.draw_from_back().expect("kan tile available");
        assert_eq!(wall.completed_rinshan_draws(), 1);
        let second = wall.draw_from_back().expect("kan tile available");

        assert_eq!(first, upper, "第一个杠该摸末尾那墩靠上的一张");
        assert_eq!(second, lower, "第二个杠才轮到压在底下的一张");
        assert_eq!(wall.completed_rinshan_draws(), 2);
        assert_eq!(wall.remaining_draws(), total - 2);
    }

    #[test]
    fn front_and_back_draws_never_hand_out_the_same_tile() {
        let mut wall = Wall::new(seat(3), &fixed_seed(13));
        let mut seen = HashSet::new();
        let mut from_back = true;

        loop {
            let tile = if from_back {
                match wall.draw_from_back() {
                    Ok(tile) => tile,
                    Err(WallError::Exhausted) => break,
                }
            } else {
                match wall.draw() {
                    Some(tile) => tile,
                    None => break,
                }
            };
            assert!(seen.insert(tile.id()));
            from_back = !from_back;
        }

        assert_eq!(seen.len(), WALL_TILE_COUNT - TILES_PER_STACK);
        assert_eq!(wall.draw_from_back(), Err(WallError::Exhausted));
    }
}
