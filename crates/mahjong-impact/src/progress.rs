//! 牌桌进度。
//!
//! 冲击麻将没有场风、自风、场数与本场：整场只有一节，从东家起庄，
//! 庄家和牌就连庄，闲家和牌庄家轮转、连庄清零。荒牌本局不算，同一庄重开。
//! 所以这里要记的只有「谁是庄」和「连庄了几次」。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::config::SEAT_COUNT;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seat(u8);

impl Seat {
    pub const COUNT: u8 = SEAT_COUNT;

    pub const fn new(index: u8) -> Result<Self, ProgressError> {
        if index < Self::COUNT {
            Ok(Self(index))
        } else {
            Err(ProgressError::InvalidSeat { index })
        }
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// 下家（逆时针的下一位）。
    #[must_use]
    pub const fn next(self) -> Self {
        Self((self.0 + 1) % Self::COUNT)
    }

    /// 从本座位往后数 `offset` 位。
    #[must_use]
    pub const fn offset_by(self, offset: u8) -> Self {
        Self((self.0 + offset % Self::COUNT) % Self::COUNT)
    }

    /// 从 `self` 到 `other` 相隔几位（0 表示同一座位）。
    #[must_use]
    pub const fn distance_to(self, other: Self) -> u8 {
        (other.0 + Self::COUNT - self.0) % Self::COUNT
    }

    /// 四个座位，按 0、1、2、3 的顺序。
    #[must_use]
    pub fn all() -> [Self; SEAT_COUNT as usize] {
        [Self(0), Self(1), Self(2), Self(3)]
    }
}

/// 连庄次数。
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DealerStreak(u32);

impl DealerStreak {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    pub const fn checked_increment(self) -> Result<Self, ProgressError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgressError::CounterOverflow {
                counter: "dealer streak",
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableProgress {
    dealer: Seat,
    dealer_streak: DealerStreak,
}

impl TableProgress {
    #[must_use]
    pub const fn new(dealer: Seat, dealer_streak: DealerStreak) -> Self {
        Self {
            dealer,
            dealer_streak,
        }
    }

    /// 开局：东家坐庄，连庄 0 次。
    #[must_use]
    pub const fn opening(dealer: Seat) -> Self {
        Self::new(dealer, DealerStreak::ZERO)
    }

    #[must_use]
    pub const fn dealer(self) -> Seat {
        self.dealer
    }

    #[must_use]
    pub const fn dealer_streak(self) -> DealerStreak {
        self.dealer_streak
    }

    /// 庄家和牌：连庄 +1，庄位不动。
    pub fn continue_dealership(&mut self) -> Result<(), ProgressError> {
        self.dealer_streak = self.dealer_streak.checked_increment()?;
        Ok(())
    }

    /// 闲家和牌：庄位轮转到下家，连庄清零。
    pub fn rotate_dealership(&mut self) {
        self.dealer = self.dealer.next();
        self.dealer_streak = DealerStreak::ZERO;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressError {
    InvalidSeat { index: u8 },
    CounterOverflow { counter: &'static str },
}

impl Display for ProgressError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSeat { index } => {
                write!(
                    formatter,
                    "seat index {index} is outside a {SEAT_COUNT}-player table"
                )
            }
            Self::CounterOverflow { counter } => write!(formatter, "{counter} counter overflow"),
        }
    }
}

impl Error for ProgressError {}

#[cfg(test)]
mod tests {
    use super::{DealerStreak, ProgressError, Seat, TableProgress};

    fn seat(index: u8) -> Seat {
        Seat::new(index).expect("valid seat")
    }

    #[test]
    fn seats_are_bounded_to_four_players() {
        assert!(Seat::new(3).is_ok());
        assert_eq!(Seat::new(4), Err(ProgressError::InvalidSeat { index: 4 }));
    }

    #[test]
    fn seats_walk_counter_clockwise_and_wrap() {
        assert_eq!(seat(3).next(), seat(0));
        assert_eq!(seat(1).offset_by(3), seat(0));
        assert_eq!(seat(2).offset_by(0), seat(2));
        assert_eq!(seat(3).distance_to(seat(1)), 2);
        assert_eq!(seat(1).distance_to(seat(1)), 0);
    }

    #[test]
    fn dealer_win_keeps_the_seat_and_counts_the_streak() {
        let mut progress = TableProgress::opening(seat(0));

        progress.continue_dealership().expect("streak 1");
        progress.continue_dealership().expect("streak 2");

        assert_eq!(progress.dealer(), seat(0));
        assert_eq!(progress.dealer_streak().value(), 2);
    }

    #[test]
    fn non_dealer_win_rotates_and_clears_the_streak() {
        let mut progress = TableProgress::opening(seat(0));
        progress.continue_dealership().expect("streak 1");

        progress.rotate_dealership();

        assert_eq!(progress.dealer(), seat(1));
        assert_eq!(progress.dealer_streak(), DealerStreak::ZERO);
    }

    #[test]
    fn streak_counter_detects_overflow() {
        assert_eq!(
            DealerStreak::new(u32::MAX).checked_increment(),
            Err(ProgressError::CounterOverflow {
                counter: "dealer streak",
            })
        );
    }
}
