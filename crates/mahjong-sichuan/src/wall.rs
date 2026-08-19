//! 牌山：骰子决定开门位与换三张方向。
//!
//! 四川麻将没有财神、没有王牌区，108 张牌全部可摸。摸牌逻辑与立直/冲击一致：
//! 一副洗好的平铺牌，从下标 0 顺序摸到尾，杠张从摸牌序列的**末尾**取（先上层后下层）。
//! 「东西各 14 墩、南北各 13 墩」以及「留出较小骰点墩」只是物理摆放的说法，
//! 牌本来就是现洗的，从哪儿断开对随机性没有任何影响，所以摸牌顺序不需要按墙遍历。
//!
//! 骰子点数和有两个用途：开门位（`break_seat`，只用于展示）与换三张方向（见
//! [`ExchangeDirection`]）。

use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

use crate::progress::Seat;
use crate::tile::{Tile, TileId, full_tile_set};

const WALL_SEED_SIZE: usize = 32;

/// 一座牌山的总张数：27 种 × 4 张。
pub const WALL_TILE_COUNT: usize = 108;
/// 总墩数（东西各 14、南北各 13）。
pub const TOTAL_STACKS: usize = 54;
/// 每个座位的墙墩数：0/2 号（东西）14 墩，1/3 号（南北）13 墩。
pub const STACKS_BY_SEAT: [usize; 4] = [14, 13, 14, 13];

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

/// 两颗骰子。点数和决定开门位与换三张方向。
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
}

/// 换三张的方向。骰和决定方向：顺/逆时针时四家沿整圈传递，对家时两两互换。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExchangeDirection {
    /// 逆时针（下家）：0→1→2→3→0。
    CounterClockwise,
    /// 顺时针（上家）：0→3→2→1→0。
    Clockwise,
    /// 对家：(0,2)(1,3) 对换。
    Opposite,
}

impl ExchangeDirection {
    /// 骰点数和 → 方向：
    /// 2、6、10 逆时针；4、8、12 顺时针；3、5、7、9、11 对家。
    #[must_use]
    pub const fn from_dice_sum(sum: u8) -> Self {
        match sum {
            2 | 6 | 10 => Self::CounterClockwise,
            4 | 8 | 12 => Self::Clockwise,
            _ => Self::Opposite,
        }
    }

    /// 这个方向下，这一家交出的牌传给谁。
    #[must_use]
    pub const fn recipient_of(self, seat: Seat) -> Seat {
        let index = seat.index();
        let recipient = match self {
            Self::CounterClockwise => (index + 1) % 4,
            Self::Clockwise => (index + 3) % 4,
            Self::Opposite => index ^ 2,
        };
        Seat(recipient)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CounterClockwise => "counter_clockwise",
            Self::Clockwise => "clockwise",
            Self::Opposite => "opposite",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wall {
    /// 按物理位置排好的 108 张牌，摸牌不会改动它。
    tiles: Box<[Tile]>,
    /// 可摸位置在 `tiles` 中的下标，按摸牌先后排（四川麻将无死牌，即 0..108）。
    order: Box<[usize]>,
    front: usize,
    back: usize,
    dice: Dice,
    break_seat: Seat,
}

impl Wall {
    /// 洗牌、掷骰、定开门位。
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

        let order: Box<[usize]> = (0..WALL_TILE_COUNT).collect::<Vec<_>>().into_boxed_slice();
        let back = order.len();

        Self {
            tiles: tiles.into_boxed_slice(),
            order,
            front: 0,
            back,
            dice,
            break_seat,
        }
    }

    #[must_use]
    pub const fn dice(&self) -> Dice {
        self.dice
    }

    /// 换三张方向（由骰点数和推出）。
    #[must_use]
    pub const fn exchange_direction(&self) -> ExchangeDirection {
        ExchangeDirection::from_dice_sum(self.dice.sum())
    }

    /// 开门位：割目家（骰子和数到的座位）。
    #[must_use]
    pub const fn break_seat(&self) -> Seat {
        self.break_seat
    }

    /// 牌山还剩几张可摸（杠张会一并扣掉）。
    #[must_use]
    pub const fn remaining_draws(&self) -> usize {
        self.back.saturating_sub(self.front)
    }

    /// 已经从牌山末端摸走的岭上牌数量。
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

    /// 杠张：从摸牌序列的**末尾**取，一墩之内先上层后下层。
    ///
    /// 摸牌序列里一墩是「先上层后下层」相邻的两张，直接从尾巴上弹就成了先摸底下那张。
    /// 所以每摸到新的一墩就把这一墩的两张对调一下再弹：弹出来的是上层，剩下的下层
    /// 留在原地，下一个杠（或者正常摸牌摸到这儿）再拿。
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

    use super::{Dice, ExchangeDirection, WALL_TILE_COUNT, Wall, WallError, WallSeed};
    use crate::progress::Seat;
    use crate::tile::full_tile_set;

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    fn fixed_seed(value: u8) -> WallSeed {
        WallSeed::from_bytes([value; 32])
    }

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
        }
    }

    #[test]
    fn break_seat_counts_the_dealer_as_one_counter_clockwise() {
        assert_eq!(ordered_wall(seat(0), Dice::new(1, 1)).break_seat(), seat(1));
        assert_eq!(ordered_wall(seat(0), Dice::new(2, 3)).break_seat(), seat(0));
        assert_eq!(ordered_wall(seat(2), Dice::new(3, 4)).break_seat(), seat(0));
        assert_eq!(ordered_wall(seat(3), Dice::new(6, 6)).break_seat(), seat(2));
    }

    #[test]
    fn exchange_direction_follows_the_dice_sum() {
        for sum in [2, 6, 10] {
            assert_eq!(
                ExchangeDirection::from_dice_sum(sum),
                ExchangeDirection::CounterClockwise
            );
        }
        for sum in [4, 8, 12] {
            assert_eq!(
                ExchangeDirection::from_dice_sum(sum),
                ExchangeDirection::Clockwise
            );
        }
        for sum in [3, 5, 7, 9, 11] {
            assert_eq!(
                ExchangeDirection::from_dice_sum(sum),
                ExchangeDirection::Opposite
            );
        }
    }

    #[test]
    fn exchange_direction_has_half_opposite_and_quarter_each_side() {
        let mut counter_clockwise = 0;
        let mut clockwise = 0;
        let mut opposite = 0;
        for first in 1..=6 {
            for second in 1..=6 {
                match ExchangeDirection::from_dice_sum(first + second) {
                    ExchangeDirection::CounterClockwise => counter_clockwise += 1,
                    ExchangeDirection::Clockwise => clockwise += 1,
                    ExchangeDirection::Opposite => opposite += 1,
                }
            }
        }
        assert_eq!(counter_clockwise, 9);
        assert_eq!(clockwise, 9);
        assert_eq!(opposite, 18);
    }

    #[test]
    fn exchange_direction_routes_every_seat_to_one_recipient() {
        for direction in [
            ExchangeDirection::CounterClockwise,
            ExchangeDirection::Clockwise,
            ExchangeDirection::Opposite,
        ] {
            let mut recipients: Vec<Seat> = Seat::all()
                .into_iter()
                .map(|seat| direction.recipient_of(seat))
                .collect();
            // 每家都交给另一家，并且四家各自恰好收到一份。
            for seat in Seat::all() {
                assert_ne!(direction.recipient_of(seat), seat, "不能和自己换");
            }
            recipients.sort_by_key(|seat| seat.index());
            assert_eq!(recipients, Seat::all());
        }
        assert_eq!(
            ExchangeDirection::CounterClockwise.recipient_of(seat(1)),
            seat(2),
        );
        assert_eq!(ExchangeDirection::Clockwise.recipient_of(seat(3)), seat(2),);
    }

    #[test]
    fn draw_order_is_plain_sequential() {
        let mut wall = ordered_wall(seat(0), Dice::new(2, 5));

        let drawn: Vec<_> = (0..3)
            .map(|_| wall.draw().expect("wall is not empty"))
            .collect();
        let expected: Vec<_> = (0..3).map(|index| wall.ordered_tiles()[index]).collect();
        assert_eq!(drawn, expected, "开头三张就是下标 0、1、2");
    }

    #[test]
    fn every_tile_is_drawable() {
        let mut wall = Wall::new(seat(1), &fixed_seed(11));
        let mut drawn = HashSet::new();

        assert_eq!(wall.remaining_draws(), WALL_TILE_COUNT);
        while let Some(tile) = wall.draw() {
            assert!(drawn.insert(tile.id()), "同一张牌被摸了两次");
        }

        assert_eq!(drawn.len(), WALL_TILE_COUNT);
        assert_eq!(wall.remaining_draws(), 0);
        assert_eq!(wall.draw(), None);
    }

    #[test]
    fn kan_tiles_take_the_upper_tile_of_the_last_stack_first() {
        let mut wall = Wall::new(seat(0), &fixed_seed(12));
        let total = wall.remaining_draws();
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

        assert_eq!(seen.len(), WALL_TILE_COUNT);
        assert_eq!(wall.draw_from_back(), Err(WallError::Exhausted));
    }
}
