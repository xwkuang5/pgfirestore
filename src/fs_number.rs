use crate::FsError;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, str::FromStr};

type Result<T> = std::result::Result<T, FsError>;

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub enum FsNumber {
    NAN,
    NegativeInfinity,
    Number(serde_json::Number),
    PositiveInfinity,
}
fn cmp_i64_f64(left: i64, right: f64) -> Ordering {
    // TODO(louiskuang): this cast can lose precision
    let left_as_f64: f64 = left as f64;
    left_as_f64.total_cmp(&right)
}

fn cmp_u64_f64(left: u64, right: f64) -> Ordering {
    // TODO(louiskuang): this cast can lose precision
    let left_as_f64: f64 = left as f64;
    left_as_f64.total_cmp(&right)
}

impl From<serde_json::Number> for FsNumber {
    fn from(number: serde_json::Number) -> Self {
        FsNumber::Number(number)
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
                if left.is_i64() {
                    let left_as_i64 = left.as_i64().unwrap();
                    if right.is_i64() {
                        return left_as_i64.cmp(&right.as_i64().unwrap());
                    } else if right.is_u64() {
                        let right_as_u64 = right.as_u64().unwrap();
                        let right_as_i64: std::result::Result<i64, _> = right_as_u64.try_into();
                        return match right_as_i64 {
                            Ok(value) => left_as_i64.cmp(&value),
                            Err(_) => Ordering::Less,
                        };
                    } else if right.is_f64() {
                        return cmp_i64_f64(left_as_i64, right.as_f64().unwrap());
                    }
                    panic!("impossible")
                } else if left.is_u64() {
                    let left_as_u64 = left.as_u64().unwrap();
                    if right.is_u64() {
                        return left_as_u64.cmp(&right.as_u64().unwrap());
                    } else if right.is_i64() {
                        let right_as_i64 = right.as_i64().unwrap();
                        let left_as_i64: std::result::Result<i64, _> = left_as_u64.try_into();
                        return match left_as_i64 {
                            Ok(value) => value.cmp(&right_as_i64),
                            Err(_) => Ordering::Greater,
                        };
                    } else if right.is_f64() {
                        return cmp_u64_f64(left_as_u64, right.as_f64().unwrap());
                    }
                    panic!("impossible")
                } else if left.is_f64() {
                    let left_as_f64 = left.as_f64().unwrap();
                    if right.is_f64() {
                        return left_as_f64.total_cmp(&right.as_f64().unwrap());
                    } else if right.is_i64() {
                        return cmp_i64_f64(right.as_i64().unwrap(), left_as_f64).reverse();
                    } else if right.is_u64() {
                        return cmp_u64_f64(right.as_u64().unwrap(), left_as_f64).reverse();
                    }
                }
                panic!("impossible")
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
