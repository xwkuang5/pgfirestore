use pgrx::prelude::*;
use pgrx::{InOutFuncs, StringInfo};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, str::FromStr};

mod fs_error;
mod fs_number;

use fs_error::FsError;
use fs_number::FsNumber;

type Result<T> = std::result::Result<T, FsError>;

pgrx::pg_module_magic!();

/**
 * core::ffi::CStr JSON format
 *
 * Null: {
 *  type: "NULL",
 *  value: null,
 * }
 *
 * Boolean: {
 *  type: "BOOLEAN",
 *  value: true,
 * }
 *
 * Number: {
 *  type: "NUMBER",
 *  value: 1,
 * }
 *
 * Date: {
 *  type: "DATE",
 *  value: 1
 * }
 *
 * String: {
 *  type: "STRING",
 *  value: "hello world"
 * }
 *
 * Bytes: {
 *  type: "BYTES",
 *  value: "0x1234"
 * }
 *
 * Reference: {
 *  type: "REFERENCE",
 *  value: "/projects/test-project/databases/test-database/documents/Users/1"
 * }
 *
 * Geo point: {
 *  type: "GEOPOINT",
 *  value: [1.0, 2.0]
 * }
 *
 * Array: {
 *  type: "ARRAY",
 *  value: [object]
 * }
 *
 * Map: {
 *  type: "MAP",
 *  value: object
 * }
 */

/**
 * pgfirestore=# select pg_column_size(fs_value_string('hello world')), pg_column_size('{"String":"hello world"}'::text), pg_column_size('{"String":"hello world"}'::json), pg_column_size('{"String":"hello world"}'::jsonb);
 * pg_column_size | pg_column_size | pg_column_size | pg_column_size
 * ---------------+----------------+----------------+----------------
 *             24 |             28 |             28 |             33
 */

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
#[inoutfuncs]
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

impl InOutFuncs for FsValue {
    fn input(input: &core::ffi::CStr) -> Self
    where
        Self: Sized,
    {
        let value = serde_json::from_str::<Value>(
            input
                .to_str()
                .expect(&format!("Failed to parse cstring as a UTF-8 string")),
        )
        .expect("Failed to parse cstring as a serde_json object");
        match FsValue::from(value) {
            Ok(value) => value,
            Err(error) => panic!("{}", error),
        }
    }

    fn output(&self, buffer: &mut StringInfo) {
        let json_repr = match self {
            FsValue::NULL => json!({
                "type": "NULL",
                "value": null,
            }),
            FsValue::Boolean(boolean) => json!({
                "type": "BOOLEAN",
                "value": boolean,
            }),
            FsValue::Number(fs_number) => {
                match fs_number {
                    FsNumber::NAN => json!({
                        "type": "NUMBER",
                        "value": "NaN",
                    }),
                    FsNumber::PositiveInfinity => json!({
                        "type": "NUMBER",
                        "value": "Infinity",
                    }),
                    FsNumber::NegativeInfinity => json!({
                        "type": "NUMBER",
                        "value": "-Infinity",
                    }),
                    FsNumber::Number(number) => json!({
                        "type": "NUMBER",
                        "value": number,
                    })
                }
            }
            _ => panic!("Unsupported FsValue"),
        };
        buffer.push_str(json_repr.to_string().as_str())
    }
}

impl FsValue {
    fn from(json_value: Value) -> Result<FsValue> {
        let json_value_as_object = json_value
            .as_object()
            .expect(&format!("Expecting a JSON object but got {}", json_value));
        let fs_value_type = json_value_as_object
            .get("type")
            .ok_or(FsError::InvalidValue(format!(
                "Expecting field 'type' in object. Found: {}",
                json_value.to_string()
            )))?;
        let fs_value_type_string = fs_value_type.as_str().expect(&format!(
            "Expecting string value for field 'type' but found {}",
            fs_value_type
        ));
        let fs_value = json_value_as_object
            .get("value")
            .ok_or(FsError::InvalidValue(format!(
                "Expecting field 'value' in object. Found: {}",
                json_value.to_string()
            )))?;

        match fs_value_type_string {
            "NULL" => FsValue::from_null_value(&fs_value),
            "BOOLEAN" => FsValue::from_boolean_value(&fs_value),
            "NUMBER" => FsValue::from_number_value(&fs_value),
            _ => Err(FsError::InvalidType(format!(
                "Firestore does not support value of type '{}'",
                fs_value_type_string
            ))),
        }
    }

    fn from_null_value(value: &Value) -> Result<FsValue> {
        if value.eq(&Value::Null) {
            Ok(FsValue::NULL)
        } else {
            Err(FsError::InvalidValue(format!(
                "Failed to parse {} as a null fsvalue",
                value
            )))
        }
    }

    fn from_boolean_value(value: &Value) -> Result<FsValue> {
        let boolean_value = value.as_bool().ok_or(FsError::InvalidValue(format!(
            "Failed to parse {} as a boolean fsvalue",
            value
        )))?;
        Ok(FsValue::Boolean(boolean_value))
    }

    fn from_number_value(value: &Value) -> Result<FsValue> {
        match value {
            serde_json::Value::Number(number) => Ok(FsValue::Number(FsNumber::from(number.clone()))),
            _ => Err(FsError::InvalidValue(format!("Expecting a JSON number but found {}", value)))
        }
    }
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
            Err(error) => panic!("{}", error),
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
                panic!(
                    "Failed to parse latitude ('{}') and longtitude ('{}') as a Geo point",
                    lat_, long_
                )
            }
            FsValue::GeoPoint(FsNumber::Number(lat_), FsNumber::Number(long_))
        }
        _ => panic!("Failed to parse latitude and longtitude as a Geo point"),
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
