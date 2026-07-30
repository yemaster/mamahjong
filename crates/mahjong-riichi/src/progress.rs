use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::RiichiVariant;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Wind {
    East = 0,
    South = 1,
    West = 2,
    North = 3,
}

impl Wind {
    const fn from_offset(offset: u8) -> Option<Self> {
        match offset {
            0 => Some(Self::East),
            1 => Some(Self::South),
            2 => Some(Self::West),
            3 => Some(Self::North),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seat(u8);

impl Seat {
    pub const fn new(variant: RiichiVariant, index: u8) -> Result<Self, ProgressError> {
        let seat_count = variant.seat_count().value();
        if index < seat_count {
            Ok(Self(index))
        } else {
            Err(ProgressError::InvalidSeat { index, seat_count })
        }
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RoundNumber(u8);

impl RoundNumber {
    pub const fn new(variant: RiichiVariant, value: u8) -> Result<Self, ProgressError> {
        let maximum = variant.seat_count().value();
        if value >= 1 && value <= maximum {
            Ok(Self(value))
        } else {
            Err(ProgressError::InvalidRoundNumber { value, maximum })
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Honba(u32);

impl Honba {
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
            None => Err(ProgressError::CounterOverflow { counter: "honba" }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiichiSticks(u32);

impl RiichiSticks {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    pub const fn checked_deposit(self) -> Result<Self, ProgressError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgressError::CounterOverflow {
                counter: "riichi sticks",
            }),
        }
    }

    #[must_use]
    pub const fn cleared(self) -> Self {
        let _ = self;
        Self::ZERO
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableProgress {
    variant: RiichiVariant,
    round_wind: Wind,
    round_number: RoundNumber,
    dealer: Seat,
    honba: Honba,
    riichi_sticks: RiichiSticks,
}

impl TableProgress {
    pub const fn new(
        variant: RiichiVariant,
        round_wind: Wind,
        round_number: RoundNumber,
        dealer: Seat,
        honba: Honba,
        riichi_sticks: RiichiSticks,
    ) -> Result<Self, ProgressError> {
        let seat_count = variant.seat_count().value();
        if dealer.index() >= seat_count {
            return Err(ProgressError::InvalidSeat {
                index: dealer.index(),
                seat_count,
            });
        }
        if round_number.value() == 0 || round_number.value() > seat_count {
            return Err(ProgressError::InvalidRoundNumber {
                value: round_number.value(),
                maximum: seat_count,
            });
        }

        Ok(Self {
            variant,
            round_wind,
            round_number,
            dealer,
            honba,
            riichi_sticks,
        })
    }

    pub fn east_one(variant: RiichiVariant, dealer: Seat) -> Result<Self, ProgressError> {
        Self::new(
            variant,
            Wind::East,
            RoundNumber::new(variant, 1)?,
            dealer,
            Honba::ZERO,
            RiichiSticks::ZERO,
        )
    }

    #[must_use]
    pub const fn variant(self) -> RiichiVariant {
        self.variant
    }

    #[must_use]
    pub const fn round_wind(self) -> Wind {
        self.round_wind
    }

    #[must_use]
    pub const fn round_number(self) -> RoundNumber {
        self.round_number
    }

    #[must_use]
    pub const fn dealer(self) -> Seat {
        self.dealer
    }

    #[must_use]
    pub const fn honba(self) -> Honba {
        self.honba
    }

    #[must_use]
    pub const fn riichi_sticks(self) -> RiichiSticks {
        self.riichi_sticks
    }

    pub const fn seat_wind(self, seat: Seat) -> Result<Wind, ProgressError> {
        let seat_count = self.variant.seat_count().value();
        if seat.index() >= seat_count {
            return Err(ProgressError::InvalidSeat {
                index: seat.index(),
                seat_count,
            });
        }

        let offset = (seat.index() + seat_count - self.dealer.index()) % seat_count;
        match Wind::from_offset(offset) {
            Some(wind) => Ok(wind),
            None => Err(ProgressError::InvalidSeat {
                index: seat.index(),
                seat_count,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressError {
    InvalidSeat { index: u8, seat_count: u8 },
    InvalidRoundNumber { value: u8, maximum: u8 },
    CounterOverflow { counter: &'static str },
}

impl Display for ProgressError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSeat { index, seat_count } => {
                write!(
                    formatter,
                    "seat index {index} is outside a {seat_count}-player table"
                )
            }
            Self::InvalidRoundNumber { value, maximum } => {
                write!(
                    formatter,
                    "round number must be between 1 and {maximum}, got {value}"
                )
            }
            Self::CounterOverflow { counter } => {
                write!(formatter, "{counter} counter overflow")
            }
        }
    }
}

impl Error for ProgressError {}

#[cfg(test)]
mod tests {
    use super::{Honba, ProgressError, RiichiSticks, RoundNumber, Seat, TableProgress, Wind};
    use crate::RiichiVariant;

    #[test]
    fn validates_seats_for_each_variant() {
        assert!(Seat::new(RiichiVariant::Yonma, 3).is_ok());
        assert!(Seat::new(RiichiVariant::Sanma, 2).is_ok());
        assert_eq!(
            Seat::new(RiichiVariant::Sanma, 3),
            Err(ProgressError::InvalidSeat {
                index: 3,
                seat_count: 3,
            })
        );
    }

    #[test]
    fn validates_round_number_for_each_variant() {
        assert!(RoundNumber::new(RiichiVariant::Yonma, 4).is_ok());
        assert!(RoundNumber::new(RiichiVariant::Sanma, 3).is_ok());
        assert!(RoundNumber::new(RiichiVariant::Sanma, 0).is_err());
        assert!(RoundNumber::new(RiichiVariant::Sanma, 4).is_err());
    }

    #[test]
    fn yonma_winds_rotate_from_dealer() {
        let dealer = Seat::new(RiichiVariant::Yonma, 2).expect("valid dealer");
        let progress =
            TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("valid progress");

        assert_eq!(progress.seat_wind(dealer), Ok(Wind::East));
        assert_eq!(
            progress.seat_wind(Seat::new(RiichiVariant::Yonma, 3).expect("valid seat")),
            Ok(Wind::South)
        );
        assert_eq!(
            progress.seat_wind(Seat::new(RiichiVariant::Yonma, 0).expect("valid seat")),
            Ok(Wind::West)
        );
        assert_eq!(
            progress.seat_wind(Seat::new(RiichiVariant::Yonma, 1).expect("valid seat")),
            Ok(Wind::North)
        );
    }

    #[test]
    fn sanma_has_east_south_and_west_seats() {
        let dealer = Seat::new(RiichiVariant::Sanma, 1).expect("valid dealer");
        let progress =
            TableProgress::east_one(RiichiVariant::Sanma, dealer).expect("valid progress");

        assert_eq!(progress.seat_wind(dealer), Ok(Wind::East));
        assert_eq!(
            progress.seat_wind(Seat::new(RiichiVariant::Sanma, 2).expect("valid seat")),
            Ok(Wind::South)
        );
        assert_eq!(
            progress.seat_wind(Seat::new(RiichiVariant::Sanma, 0).expect("valid seat")),
            Ok(Wind::West)
        );
    }

    #[test]
    fn table_progress_revalidates_variant_bound_values() {
        let yonma_four = Seat::new(RiichiVariant::Yonma, 3).expect("valid yonma seat");
        let yonma_round_four =
            RoundNumber::new(RiichiVariant::Yonma, 4).expect("valid yonma round");

        assert!(TableProgress::east_one(RiichiVariant::Sanma, yonma_four).is_err());
        assert!(
            TableProgress::new(
                RiichiVariant::Sanma,
                Wind::East,
                yonma_round_four,
                Seat::new(RiichiVariant::Sanma, 0).expect("valid sanma seat"),
                Honba::ZERO,
                RiichiSticks::ZERO,
            )
            .is_err()
        );
    }

    #[test]
    fn counters_detect_overflow() {
        assert_eq!(
            Honba::new(u32::MAX).checked_increment(),
            Err(ProgressError::CounterOverflow { counter: "honba" })
        );
        assert_eq!(
            RiichiSticks::new(u32::MAX).checked_deposit(),
            Err(ProgressError::CounterOverflow {
                counter: "riichi sticks",
            })
        );
    }

    #[test]
    fn east_one_starts_without_honba_or_riichi_sticks() {
        let dealer = Seat::new(RiichiVariant::Yonma, 0).expect("valid dealer");
        let progress =
            TableProgress::east_one(RiichiVariant::Yonma, dealer).expect("valid progress");

        assert_eq!(progress.round_wind(), Wind::East);
        assert_eq!(progress.round_number().value(), 1);
        assert_eq!(progress.dealer(), dealer);
        assert_eq!(progress.honba(), Honba::ZERO);
        assert_eq!(progress.riichi_sticks(), RiichiSticks::ZERO);
    }
}
