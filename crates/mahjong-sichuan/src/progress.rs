//! 牌桌进度。
//!
//! 四川麻将血战到底打 4 局：首局庄家是东，之后庄家是上一局第一个胡者。
//! 没有场风、自风、本场、连庄——这里只需要记「打到第几局」和「本局谁坐庄」。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::config::{HAND_COUNT, SEAT_COUNT};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seat(pub(crate) u8);

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

/// 对局进度：第几局 + 谁坐庄。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableProgress {
    hand_index: u8,
    dealer: Seat,
}

impl TableProgress {
    /// 开局：第 1 局（下标 0），东家（0 号座位）坐庄。
    #[must_use]
    pub const fn opening() -> Self {
        Self {
            hand_index: 0,
            dealer: Seat(0),
        }
    }

    #[must_use]
    pub const fn hand_index(self) -> u8 {
        self.hand_index
    }

    #[must_use]
    pub const fn dealer(self) -> Seat {
        self.dealer
    }

    /// 是否已经是最后一局（4 局制的第 4 局，下标 3）。
    #[must_use]
    pub const fn is_final_hand(self) -> bool {
        self.hand_index + 1 >= HAND_COUNT
    }

    /// 推进到下一局：庄家换成上一局第一个胡者；若上一局无人胡牌（流局 0 家胡），
    /// 庄家保持不变。
    ///
    /// # Errors
    ///
    /// 已经打完 4 局。
    pub fn advance(&mut self, first_winner: Option<Seat>) -> Result<(), ProgressError> {
        self.hand_index = self
            .hand_index
            .checked_add(1)
            .ok_or(ProgressError::CounterOverflow {
                counter: "hand index",
            })?;
        if let Some(winner) = first_winner {
            self.dealer = winner;
        }
        Ok(())
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
    use super::{ProgressError, Seat, TableProgress};

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
    fn the_opening_table_starts_at_hand_zero_with_east_dealing() {
        let progress = TableProgress::opening();

        assert_eq!(progress.hand_index(), 0);
        assert_eq!(progress.dealer(), seat(0));
        assert!(!progress.is_final_hand());
    }

    #[test]
    fn the_next_dealer_is_the_first_winner_of_the_previous_hand() {
        let mut progress = TableProgress::opening();

        progress.advance(Some(seat(2))).expect("hand advances");

        assert_eq!(progress.hand_index(), 1);
        assert_eq!(progress.dealer(), seat(2));
    }

    #[test]
    fn a_void_hand_keeps_the_same_dealer() {
        let mut progress = TableProgress::opening();

        progress.advance(None).expect("hand advances");

        assert_eq!(progress.hand_index(), 1);
        assert_eq!(progress.dealer(), seat(0));
    }

    #[test]
    fn the_fourth_hand_is_final() {
        let mut progress = TableProgress::opening();
        for _ in 0..3 {
            progress.advance(Some(seat(1))).expect("hand advances");
        }

        assert_eq!(progress.hand_index(), 3);
        assert!(progress.is_final_hand());
    }
}
