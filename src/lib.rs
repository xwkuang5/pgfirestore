use std::{collections::HashMap, str::FromStr, cmp::Ordering};
use pgrx::prelude::*;
use serde::{Deserialize, Serialize};

pgrx::pg_module_magic!();

/**
 * pgfirestore=# select pg_column_size(fs_value_string('hello world')), pg_column_size('{"String":"hello world"}'::text), pg_column_size('{"String":"hello world"}'::json), pg_column_size('{"String":"hello world"}'::jsonb);
 * pg_column_size | pg_column_size | pg_column_size | pg_column_size
 * ---------------+----------------+----------------+----------------
 *             24 |             28 |             28 |             33
 */

#[derive(PostgresType, PostgresEq, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub enum FsValue {
    NULL,
    Boolean(bool),
    NAN,
    Number(serde_json::Number),
    Date(pgrx::Date),
    String(String),
    Bytes(Vec<u8>),
    Reference(String),
    // f64 does not implement Eq because NaN != NaN
    GeoPoint(serde_json::Number, serde_json::Number),
    Array(Vec<FsValue>),
    Map(HashMap<String, FsValue>),
}

// TODO(louiskuang): Ord requires PartialOrd but serde_json::Number is not happy about it.
// impl Ord for FsValue {
//     fn cmp(&self, other: &Self) -> Ordering {
//         Ordering::Greater
//     }
// }

#[pg_extern]
fn fs_value_null() -> FsValue {
    FsValue::NULL
}

#[pg_extern]
fn fs_value_boolean(value: bool) -> FsValue {
    FsValue::Boolean(value)
}

#[pg_extern]
fn fs_value_nan() -> FsValue {
    FsValue::NAN
}

#[pg_extern]
fn fs_value_number(cstr: &core::ffi::CStr) -> FsValue {
    match cstr.to_str() {
        Ok(str) => match serde_json::Number::from_str(str) {
            Ok(value) => FsValue::Number(value),
            Err(error) => panic!("Failed to parse cstring as a serde_json Number: {}", error),
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
fn fs_value_samples() -> Vec<FsValue> {
    vec![
        FsValue::NULL,
        FsValue::Boolean(true),
        FsValue::NAN,
        FsValue::Number(serde_json::Number::from(7)),
        FsValue::Date(pgrx::Date::from(0)),
        FsValue::String(String::from("hello")),
        FsValue::Bytes(vec![0x00, 0x01]),
        FsValue::Reference(String::from(
            "/projects/test-project/databases/test-database/documents/Users/1",
        )),
        FsValue::GeoPoint(
            serde_json::Number::from_f64(1.0).unwrap(),
            serde_json::Number::from_f64(2.0).unwrap(),
        ),
        FsValue::Array(vec![FsValue::NULL]),
        FsValue::Map(HashMap::from([(String::from("a"), FsValue::NULL)])),
    ]
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_fs_value() {
        assert_eq!(crate::FsValue::NULL, crate::fs_value_null());
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
