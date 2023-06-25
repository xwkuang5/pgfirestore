use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap, str::FromStr};

pgrx::pg_module_magic!();

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
                        let right_as_i64: Result<i64, _> = right_as_u64.try_into();
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
                        let left_as_i64: Result<i64, _> = left_as_u64.try_into();
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

#[derive(Debug, PartialEq, Eq)]
pub struct ParseFsNumberError(String);

impl FromStr for FsNumber {
    type Err = ParseFsNumberError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NaN" => Ok(FsNumber::NAN),
            "-Infinity" => Ok(FsNumber::NegativeInfinity),
            "Infinity" => Ok(FsNumber::PositiveInfinity),
            _ => match serde_json::Number::from_str(s) {
                Ok(number) => Ok(FsNumber::Number(number)),
                Err(error) => Err(ParseFsNumberError(format!(
                    "Failed to parse cstring ('{}') as a FsNumber: {}",
                    s, error
                ))),
            },
        }
    }
}

/**
 * pgfirestore=# select pg_column_size(fs_value_string('hello world')), pg_column_size('{"String":"hello world"}'::text), pg_column_size('{"String":"hello world"}'::json), pg_column_size('{"String":"hello world"}'::jsonb);
 * pg_column_size | pg_column_size | pg_column_size | pg_column_size
 * ---------------+----------------+----------------+----------------
 *             24 |             28 |             28 |             33
 */

// TODO(louiskuang): implement custom input & output function based on json
#[derive(
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    Debug,
    Clone,
    PostgresType,
    PostgresEq,
    PostgresOrd,
)]
pub enum FsValue {
    NULL,
    Boolean(bool),
    Number(FsNumber),
    Date(pgrx::Date),
    String(String),
    Bytes(Vec<u8>),
    Reference(String),
    // f64 does not implement Eq because NaN != NaN
    GeoPoint(FsNumber, FsNumber),
    Array(Vec<FsValue>),
    Map(BTreeMap<String, FsValue>),
}

#[pg_extern]
fn fs_value_null() -> FsValue {
    FsValue::NULL
}

#[pg_extern]
fn fs_value_boolean(value: bool) -> FsValue {
    FsValue::Boolean(value)
}

#[pg_extern]
fn fs_value_number(cstr: &core::ffi::CStr) -> FsValue {
    match cstr.to_str() {
        Ok(str) => match FsNumber::from_str(str) {
            Ok(number) => FsValue::Number(number),
            Err(ParseFsNumberError(err_string)) => panic!("{}", err_string),
        },
        Err(error) => panic!("Failed to parse cstring as a UTF-8 string: {}", error),
    }
}

#[pg_extern]
fn fs_value_string(cstr: &core::ffi::CStr) -> FsValue {
    match cstr.to_str() {
        Ok(str) => FsValue::String(String::from(str)),
        Err(error) => panic!("Failed to parse cstring as a UTF-8 string: {}", error),
    }
}

#[pg_extern]
fn fs_value_bytes(cstr: &core::ffi::CStr) -> FsValue {
    FsValue::Bytes(Vec::from(cstr.to_bytes()))
}

#[pg_extern]
fn fs_value_geo(latitude: &core::ffi::CStr, longtitude: &core::ffi::CStr) -> FsValue {
    let lat = fs_value_number(latitude);
    let long = fs_value_number(longtitude);
    match (lat, long) {
        (FsValue::Number(FsNumber::Number(lat_)), FsValue::Number(FsNumber::Number(long_))) => {
            if !lat_.is_f64() || !long_.is_f64() {
                panic!("Failed to parse latitude ('{}') and longtitude ('{}') as a Geo point", lat_, long_)
            }
            FsValue::GeoPoint(FsNumber::Number(lat_), FsNumber::Number(long_))
        },
        _ => panic!("Failed to parse latitude and longtitude as a Geo point")
    }
}


#[pg_extern]
fn fs_value_samples() -> Vec<FsValue> {
    vec![
        FsValue::NULL,
        FsValue::Boolean(true),
        FsValue::Number(FsNumber::from(serde_json::Number::from(7))),
        FsValue::Date(pgrx::Date::from(0)),
        FsValue::String(String::from("hello")),
        FsValue::Bytes(vec![0x00, 0x01]),
        FsValue::Reference(String::from(
            "/projects/test-project/databases/test-database/documents/Users/1",
        )),
        FsValue::GeoPoint(
            FsNumber::from(serde_json::Number::from_f64(1.0).unwrap()),
            FsNumber::from(serde_json::Number::from_f64(2.0).unwrap()),
        ),
        FsValue::Array(vec![FsValue::NULL]),
        FsValue::Map(BTreeMap::from([(String::from("a"), FsValue::NULL)])),
    ]
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;
    use std::ffi::CString;

    #[pg_test]
    fn test_fs_value() {
        assert_eq!(crate::FsValue::NULL, crate::fs_value_null());
    }

    #[pg_test]
    fn test_fs_number() {
        let nan = CString::new("NaN").expect("CString::new failed");
        let number_1 = CString::new("1").expect("CString::new failed");
        assert!(crate::fs_value_number(nan.as_c_str()) == crate::fs_value_number(nan.as_c_str()));
        assert!(
            crate::fs_value_number(nan.as_c_str()) < crate::fs_value_number(number_1.as_c_str())
        );
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
