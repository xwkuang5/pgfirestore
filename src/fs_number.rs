use crate::FsError;
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use std::ops::Add;
use std::{cmp::Ordering, str::FromStr};

type Result<T> = std::result::Result<T, FsError>;

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub enum FsNumber {
    NAN,
    NegativeInfinity,
    Number(serde_json::Number),
    PositiveInfinity,
}

impl From<serde_json::Number> for FsNumber {
    fn from(number: serde_json::Number) -> Self {
        FsNumber::Number(number)
    }
}

fn number_to_bigdecimal(val: &serde_json::Number) -> BigDecimal {
    // TODO(louiskuang): parsing error should be thrown at FsNumber construction time.
    BigDecimal::from_str(val.to_string().as_str()).unwrap()
}

fn number_from_bigdecimal(val: &BigDecimal) -> FsNumber {
    // TODO(louiskuang): parsing error should be thrown at FsNumber construction time.
    FsNumber::Number(serde_json::Number::from_str(val.to_string().as_str()).unwrap())
}

impl Add for FsNumber {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        match (self, other) {
            (FsNumber::NAN, _) => FsNumber::NAN,
            (_, FsNumber::NAN) => FsNumber::NAN,
            (FsNumber::NegativeInfinity, _) => FsNumber::NegativeInfinity,
            (FsNumber::PositiveInfinity, _) => FsNumber::PositiveInfinity,
            (_, FsNumber::PositiveInfinity) => FsNumber::PositiveInfinity,
            (_, FsNumber::NegativeInfinity) => FsNumber::NegativeInfinity,
            (FsNumber::Number(l), FsNumber::Number(r)) => {
                let left = number_to_bigdecimal(&l);
                let right = number_to_bigdecimal(&r);
                number_from_bigdecimal(&(left + right))
            }
        }
    }
}

impl Ord for FsNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.eq(other) {
            return Ordering::Equal;
        }
        match (&self, other) {
            (FsNumber::NAN, _) => Ordering::Less,
            (FsNumber::PositiveInfinity, _) => Ordering::Greater,
            (FsNumber::NegativeInfinity, _) => Ordering::Less,
            (FsNumber::Number(_), FsNumber::NAN) => Ordering::Greater,
            (FsNumber::Number(_), FsNumber::PositiveInfinity) => Ordering::Less,
            (FsNumber::Number(_), FsNumber::NegativeInfinity) => Ordering::Greater,
            (FsNumber::Number(left), FsNumber::Number(right)) => {
                number_to_bigdecimal(left).cmp(&number_to_bigdecimal(right))
            }
        }
    }
}

impl PartialOrd for FsNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for FsNumber {
    type Err = FsError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "NaN" => Ok(FsNumber::NAN),
            "-Infinity" => Ok(FsNumber::NegativeInfinity),
            "Infinity" => Ok(FsNumber::PositiveInfinity),
            _ => match serde_json::Number::from_str(s) {
                Ok(number) => Ok(FsNumber::Number(number)),
                Err(error) => Err(FsError::InvalidValue(format!(
                    "Failed to parse cstring ('{}') as a FsNumber: {}",
                    s, error
                ))),
            },
        }
    }
}

mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_equal() {
        assert_eq!(
            FsNumber::from_str("0")
                .unwrap()
                .cmp(&FsNumber::from_str("0").unwrap()),
            Ordering::Equal
        );
        assert_eq!(
            FsNumber::from_str("0.0")
                .unwrap()
                .cmp(&FsNumber::from_str("0.0").unwrap()),
            Ordering::Equal
        );
        assert_eq!(
            FsNumber::from_str("0")
                .unwrap()
                .cmp(&FsNumber::from_str("0.0").unwrap()),
            Ordering::Equal
        );
        assert_eq!(
            FsNumber::from_str("1")
                .unwrap()
                .cmp(&FsNumber::from_str("1.0").unwrap()),
            Ordering::Equal
        );
    }

    fn assert_lt(left: FsNumber, right: FsNumber) {
        assert_eq!(left.cmp(&right), Ordering::Less);
        assert_eq!(right.cmp(&left), Ordering::Greater);
    }

    #[test]
    fn test_lt() {
        assert_lt(
            FsNumber::from_str("0").unwrap(),
            FsNumber::from_str("1").unwrap(),
        );
        assert_lt(
            FsNumber::from_str("0.0").unwrap(),
            FsNumber::from_str("1.0").unwrap(),
        );
        assert_lt(
            FsNumber::from_str("0.5").unwrap(),
            FsNumber::from_str("1").unwrap(),
        );
        assert_lt(
            FsNumber::from_str("0").unwrap(),
            FsNumber::from_str("0.5").unwrap(),
        );
    }

    #[test]
    fn test_add() {
        assert_eq!(
            FsNumber::from_str("0").unwrap() + FsNumber::from_str("1").unwrap(),
            FsNumber::from_str("1").unwrap(),
        );
        assert_eq!(
            FsNumber::from_str("1").unwrap() + FsNumber::from_str("1").unwrap(),
            FsNumber::from_str("2").unwrap(),
        );
        assert_eq!(
            FsNumber::from_str("0.5").unwrap() + FsNumber::from_str("1").unwrap(),
            FsNumber::from_str("1.5").unwrap(),
        );
    }
}
