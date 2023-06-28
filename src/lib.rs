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
        buffer.push_str(self.to_json_value().to_string().as_str())
    }
}

impl FsValue {
    fn to_json_value(&self) -> Value {
        match &self {
            FsValue::NULL => json!({
                "type": "NULL",
                "value": null,
            }),
            FsValue::Boolean(boolean) => json!({
                "type": "BOOLEAN",
                "value": boolean,
            }),
            FsValue::Number(fs_number) => match fs_number {
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
                }),
            },
            FsValue::String(fs_string) => json!({
                "type": "STRING",
                "value": fs_string,
            }),
            FsValue::Array(fs_value_array) => {
                let mut value_array = Vec::new();
                for fs_array_element in fs_value_array.iter() {
                    value_array.push(fs_array_element.to_json_value());
                }
                json!({
                    "type": "ARRAY",
                    "value": value_array,
                })
            }
            _ => panic!("Unsupported FsValue"),
        }
    }

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
            "STRING" => FsValue::from_string_value(&fs_value),
            "ARRAY" => FsValue::from_array_value(&fs_value),
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
            serde_json::Value::Number(number) => {
                Ok(FsValue::Number(FsNumber::from(number.clone())))
            }
            _ => Err(FsError::InvalidValue(format!(
                "Expecting a JSON number but found {}",
                value
            ))),
        }
    }

    fn from_string_value(value: &Value) -> Result<FsValue> {
        let string_value = value.as_str().ok_or(FsError::InvalidValue(format!(
            "Failed to parse {} as a string fsvalue",
            value
        )))?;
        Ok(FsValue::String(string_value.to_owned()))
    }

    fn from_array_value(value: &Value) -> Result<FsValue> {
        let array_value = value.as_array().ok_or(FsError::InvalidValue(format!(
            "Failed to parse {} as an array fsvalue",
            value
        )))?;
        let mut fs_array_value = Vec::new();
        for array_element in array_value.iter() {
            fs_array_value.push(FsValue::from(array_element.to_owned())?);
        }
        Ok(FsValue::Array(fs_array_value))
    }
}

#[pg_extern]
fn fs_value_null() -> FsValue {
    FsValue::NULL
}

#[pg_extern]
fn fs_value_nan() -> FsValue {
    FsValue::Number(FsNumber::NAN)
}

#[pg_extern]
fn fs_value_boolean(value: bool) -> FsValue {
    FsValue::Boolean(value)
}

#[pg_extern]
fn fs_value_number_from_integer(value: i32) -> FsValue {
    FsValue::Number(FsNumber::Number(serde_json::Number::from(value)))
}

#[pg_extern]
fn fs_value_number_from_double(value: f64) -> FsValue {
    FsValue::Number(FsNumber::Number(
        serde_json::Number::from_f64(value)
            .expect(&format!("Failed to parse {} as a json number", value)),
    ))
}

#[pg_extern]
fn fs_value_number_from_str(cstr: &core::ffi::CStr) -> FsValue {
    match cstr.to_str() {
        Ok(str) => match FsNumber::from_str(str) {
            Ok(number) => FsValue::Number(number),
            Err(error) => panic!("{}", error),
        },
        Err(error) => panic!("Failed to parse cstring as a UTF-8 string: {}", error),
    }
}

#[pg_extern]
fn fs_value_string(string: &str) -> FsValue {
    FsValue::String(string.to_owned())
}

#[pg_extern]
fn fs_value_bytes(bytes: Vec<u8>) -> FsValue {
    FsValue::Bytes(bytes)
}

#[pg_extern]
fn fs_value_examples() -> Vec<FsValue> {
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
    use crate::*;
    use std::ffi::CString;

    #[pg_test]
    fn test_fs_nan() {
        assert_eq!(
            fs_value_number_from_str(CString::new("NaN").expect("CString::new failed").as_c_str()),
            fs_value_nan()
        );
    }

    #[pg_test]
    fn test_fs_number_from_str() {
        assert_eq!(
            fs_value_number_from_str(CString::new("1").expect("CString::new failed").as_c_str()),
            fs_value_number_from_integer(1)
        );
        assert_eq!(
            fs_value_number_from_str(CString::new("1.1").expect("CString::new failed").as_c_str()),
            fs_value_number_from_double(1.1)
        );
    }

    #[pg_test]
    fn test_fs_number() {
        assert_eq!(
            Spi::get_one::<FsValue>(r#"select '{"type": "NUMBER", "value": 1}'::fsvalue"#),
            Ok(Some(fs_value_number_from_integer(1)))
        );
        assert_eq!(
            Spi::get_one::<FsValue>(r#"select '{"type": "NUMBER", "value": 1.1}'::fsvalue"#),
            Ok(Some(fs_value_number_from_double(1.1)))
        );
    }

    #[pg_test]
    fn test_fs_string() {
        assert_eq!(
            Spi::get_one::<FsValue>(r#"select '{"type": "STRING", "value": "hello world"}'::fsvalue"#),
            Ok(Some(fs_value_string("hello world")))
        );
    }

    #[pg_test]
    fn test_fs_array() {
        let array = Spi::get_one::<FsValue>(
            r#"select '{
                "type": "ARRAY",
                "value": [
                    {"type": "NUMBER", "value": 1},
                    {"type": "NULL", "value": null},
                    {"type": "BOOLEAN", "value": true}
                ]
            }'::fsvalue;"#,
        );

        assert_eq!(
            array,
            Ok(Some(FsValue::Array(vec![
                fs_value_number_from_integer(1),
                fs_value_null(),
                fs_value_boolean(true)
            ])))
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
