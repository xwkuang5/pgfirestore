use base64::{engine::general_purpose, Engine as _};
use pgrx::prelude::*;
use pgrx::{InOutFuncs, StringInfo};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::mem;
use std::{collections::BTreeMap, str::FromStr};

mod fs_error;
mod fs_number;
mod fs_reference;

use fs_error::FsError;
use fs_number::FsNumber;
use fs_reference::FsPath;
use fs_reference::FsReference;
use fs_reference::FS_REFERENCE_ROOT;

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
 *  value: "/users/1"
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
    // TODO(louiskuang): support date type
    Date(pgrx::Date),
    String(String),
    Bytes(Vec<u8>),
    Reference(FsReference),
    // TODO(louiskuang): support geo point type
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
            FsValue::Reference(reference) => json!({
                "type": "REFERENCE",
                "value": reference.to_string(),
            }),
            FsValue::Bytes(fs_bytes) => json!({
                "type": "BYTES",
                "value": general_purpose::STANDARD.encode(fs_bytes),
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
            FsValue::Map(fs_value_map) => {
                let mut value_map = BTreeMap::new();
                for (key, value) in fs_value_map.iter() {
                    value_map.insert(key, value.to_json_value());
                }
                json!({
                    "type": "MAP",
                    "value": value_map,
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
            "REFERENCE" => FsValue::from_reference_value(&fs_value),
            "BYTES" => FsValue::from_bytes_value(&fs_value),
            "ARRAY" => FsValue::from_array_value(&fs_value),
            "MAP" => FsValue::from_map_value(&fs_value),
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
            "Failed to parse {} as a string",
            value
        )))?;
        Ok(FsValue::String(string_value.to_owned()))
    }

    fn from_reference_value(value: &Value) -> Result<FsValue> {
        let string_value = value.as_str().ok_or(FsError::InvalidValue(format!(
            "Failed to parse {} as a string",
            value
        )))?;
        FsReference::from_str(string_value).map(|reference| FsValue::Reference(reference))
    }

    fn from_bytes_value(value: &Value) -> Result<FsValue> {
        let string_value = value.as_str().ok_or(FsError::InvalidValue(format!(
            "Failed to parse {} as a string",
            value
        )))?;
        general_purpose::STANDARD
            .decode(string_value)
            .map(|bytes| FsValue::Bytes(bytes))
            .map_err(|err| {
                FsError::InvalidValue(format!(
                    "Failed to decode value as a base64 byte string: {}",
                    err
                ))
            })
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

    fn from_map_value(value: &Value) -> Result<FsValue> {
        let map_value = value.as_object().ok_or(FsError::InvalidValue(format!(
            "Failed to parse {} as a map fsvalue",
            value
        )))?;
        let mut fs_map_value = BTreeMap::new();
        for (key, value) in map_value.iter() {
            fs_map_value.insert(key.to_owned(), FsValue::from(value.to_owned())?);
        }
        Ok(FsValue::Map(fs_map_value))
    }

    fn as_reference(&self) -> Option<&FsReference> {
        match &self {
            FsValue::Reference(reference) => Some(reference),
            _ => None,
        }
    }

    fn as_map(&self) -> Option<&BTreeMap<String, FsValue>> {
        match &self {
            FsValue::Map(value) => Some(value),
            _ => None,
        }
    }
}

#[pg_extern]
fn fs_null() -> FsValue {
    FsValue::NULL
}

#[pg_extern]
fn fs_nan() -> FsValue {
    FsValue::Number(FsNumber::NAN)
}

#[pg_extern]
fn fs_boolean(value: bool) -> FsValue {
    FsValue::Boolean(value)
}

#[pg_extern]
fn fs_number_from_integer(value: i32) -> FsValue {
    FsValue::Number(FsNumber::Number(serde_json::Number::from(value)))
}

#[pg_extern]
fn fs_number_from_double(value: f64) -> FsValue {
    FsValue::Number(FsNumber::Number(
        serde_json::Number::from_f64(value)
            .expect(&format!("Failed to parse {} as a json number", value)),
    ))
}

#[pg_extern]
fn fs_number_from_str(cstr: &core::ffi::CStr) -> FsValue {
    match cstr.to_str() {
        Ok(str) => match FsNumber::from_str(str) {
            Ok(number) => FsValue::Number(number),
            Err(error) => panic!("{}", error),
        },
        Err(error) => panic!("Failed to parse cstring as a UTF-8 string: {}", error),
    }
}

#[pg_extern]
fn fs_string(string: &str) -> FsValue {
    FsValue::String(string.to_owned())
}

#[pg_extern]
fn fs_reference(string: &str) -> FsValue {
    FsValue::Reference(
        FsReference::from_str(string).expect("Failed to parse string as a reference"),
    )
}

#[pg_extern]
fn fs_bytes(bytes: Vec<u8>) -> FsValue {
    FsValue::Bytes(bytes)
}

#[pg_extern]
fn fs_array(array: Vec<FsValue>) -> FsValue {
    FsValue::Array(array)
}

#[pg_extern]
fn fs_is_valid_document_key(fs_ref: FsValue) -> bool {
    fs_ref
        .as_reference()
        .map(|reference| !reference.is_root() && reference.has_complete_path())
        .unwrap_or(false)
}

#[pg_extern]
fn fs_database_root() -> FsValue {
    FsValue::Reference(FS_REFERENCE_ROOT)
}

#[pg_extern]
fn fs_parent(reference: FsValue) -> FsValue {
    let fs_ref = reference
        .as_reference()
        .expect("expecting a reference type");
    FsValue::Reference(fs_ref.parent())
}

#[pg_extern]
fn fs_collection_id(reference: FsValue) -> String {
    let fs_ref = reference
        .as_reference()
        .expect("expecting a reference type");
    fs_ref.collection_id().to_string()
}

#[pg_extern]
fn fs_map_get(fs_map: FsValue, field_name: &str) -> Option<FsValue> {
    fs_map
        .as_map()
        .and_then(|map| map.get(field_name).map(|value| value.to_owned()))
}

fn is_same_type(lhs: &FsValue, rhs: &FsValue) -> bool {
    mem::discriminant(lhs) == mem::discriminant(rhs)
}

#[pg_extern]
fn fs_lt(lhs: FsValue, rhs: FsValue) -> bool {
    is_same_type(&lhs, &rhs) && lhs.lt(&rhs)
}

#[pg_extern]
fn fs_gt(lhs: FsValue, rhs: FsValue) -> bool {
    is_same_type(&lhs, &rhs) && lhs.gt(&rhs)
}

#[pg_extern]
fn fs_le(lhs: FsValue, rhs: FsValue) -> bool {
    is_same_type(&lhs, &rhs) && lhs.le(&rhs)
}

#[pg_extern]
fn fs_ge(lhs: FsValue, rhs: FsValue) -> bool {
    is_same_type(&lhs, &rhs) && lhs.ge(&rhs)
}

#[pg_extern]
fn fs_eq(lhs: FsValue, rhs: FsValue) -> bool {
    lhs.eq(&rhs)
}

// For any `NULL` operands, this implement the `IS_NOT_NULL` semantics
// https://cloud.google.com/firestore/docs/query-data/queries#not_equal_
#[pg_extern]
fn fs_neq(lhs: FsValue, rhs: FsValue) -> bool {
    match (lhs.eq(&fs_null()), rhs.eq(&fs_null())) {
        (true, true) => false,
        (true, _) => true,
        (_, true) => true,
        (_, _) => lhs.ne(&rhs),
    }
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
        FsValue::Reference(FsReference {
            path: FsPath(vec![]),
        }),
        FsValue::GeoPoint(
            FsNumber::from(serde_json::Number::from_f64(1.0).unwrap()),
            FsNumber::from(serde_json::Number::from_f64(2.0).unwrap()),
        ),
        FsValue::Array(vec![FsValue::NULL]),
        FsValue::Map(BTreeMap::from([(String::from("a"), FsValue::NULL)])),
    ]
}

extension_sql!(
    "\n\
        CREATE TABLE fs_documents (\n\
            reference fsvalue PRIMARY KEY, \n\
            properties fsvalue\n\
            CHECK (fs_is_valid_document_key(reference))\n\
        );\n\
    ",
    name = "main_table",
);

extension_sql!(
    "\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/1'), fs_null());\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/1/posts/1'), fs_string('hello foo'));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/1/posts/2'), fs_string('hello bar'));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/2'), fs_boolean(false));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/3'), fs_boolean(true));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/4'), fs_number_from_integer(1));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/users/5'), fs_number_from_double(1.1));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/posts/1'), fs_reference('/users/1/posts/1'));\n\
    INSERT INTO fs_documents VALUES (fs_reference('/posts/2'), fs_array(ARRAY[fs_string('hello baz')]));\n\
    ",
    name = "seed_data",
    requires = ["main_table"]
);

extension_sql!(
    "\n\
        CREATE FUNCTION fs_collection(parent fsvalue, collection_id text) \n\
        RETURNS TABLE (reference fsvalue, properties fsvalue) AS $$ \n\
            SELECT * FROM fs_documents \n\
            WHERE \n\
                fs_parent(reference) = parent AND \n\
                fs_collection_id(reference) = collection_id \n\
        $$ LANGUAGE SQL; \n\
    ",
    name = "collection_tvf",
    requires = ["main_table"],
);

extension_sql!(
    "\n\
        CREATE FUNCTION fs_collection_group(collection_id text) \n\
        RETURNS TABLE (reference fsvalue, properties fsvalue) AS $$ \n\
            SELECT * FROM fs_documents \n\
            WHERE fs_collection_id(reference) = collection_id \n\
        $$ LANGUAGE SQL; \n\
    ",
    name = "collection_group_tvf",
    requires = ["main_table"],
);

extension_sql!(
    "\n\
        CREATE OPERATOR -> ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = text, \n\
            FUNCTION = fs_map_get \n\
        ); \n\
    ",
    name = "document_get",
);

extension_sql!(
    "\n\
        CREATE OPERATOR #< ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = fsvalue, \n\
            FUNCTION = fs_lt \n\
        ); \n\
    ",
    name = "type_clamped_lt",
);

extension_sql!(
    "\n\
        CREATE OPERATOR #> ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = fsvalue, \n\
            FUNCTION = fs_gt \n\
        ); \n\
    ",
    name = "type_clamped_gt",
);

extension_sql!(
    "\n\
        CREATE OPERATOR #<= ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = fsvalue, \n\
            FUNCTION = fs_le \n\
        ); \n\
    ",
    name = "type_clamped_le",
);

extension_sql!(
    "\n\
        CREATE OPERATOR #>= ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = fsvalue, \n\
            FUNCTION = fs_ge \n\
        ); \n\
    ",
    name = "type_clamped_ge",
);

extension_sql!(
    "\n\
        CREATE OPERATOR #!= ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = fsvalue, \n\
            FUNCTION = fs_neq \n\
        ); \n\
    ",
    name = "fs_neq",
);

extension_sql!(
    "\n\
        CREATE OPERATOR #= ( \n\
            LEFTARG = fsvalue, \n\
            RIGHTARG = fsvalue, \n\
            FUNCTION = fs_eq \n\
        ); \n\
    ",
    name = "fs_eq",
);

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use crate::*;
    use std::ffi::CString;

    #[pg_test]
    fn test_fs_nan() {
        assert_eq!(
            fs_number_from_str(CString::new("NaN").expect("CString::new failed").as_c_str()),
            fs_nan()
        );
    }

    #[pg_test]
    fn test_fs_number_from_str() {
        assert_eq!(
            fs_number_from_str(CString::new("1").expect("CString::new failed").as_c_str()),
            fs_number_from_integer(1)
        );
        assert_eq!(
            fs_number_from_str(CString::new("1.1").expect("CString::new failed").as_c_str()),
            fs_number_from_double(1.1)
        );
    }

    #[pg_test]
    fn test_fs_number() {
        assert_eq!(
            Spi::get_one::<FsValue>(r#"select '{"type": "NUMBER", "value": 1}'::fsvalue"#),
            Ok(Some(fs_number_from_integer(1)))
        );
        assert_eq!(
            Spi::get_one::<FsValue>(r#"select '{"type": "NUMBER", "value": 1.1}'::fsvalue"#),
            Ok(Some(fs_number_from_double(1.1)))
        );
    }

    #[pg_test]
    fn test_fs_string() {
        assert_eq!(
            Spi::get_one::<FsValue>(
                r#"select '{"type": "STRING", "value": "hello world"}'::fsvalue"#
            ),
            Ok(Some(fs_string("hello world")))
        );
    }

    #[pg_test]
    fn test_fs_reference() {
        assert_eq!(
            Spi::get_one::<FsValue>(
                r#"select '{"type": "REFERENCE", "value": "/users/1"}'::fsvalue"#
            ),
            Ok(Some(fs_reference("/users/1")))
        );
    }

    #[pg_test]
    fn test_fs_bytes() {
        assert_eq!(
            Spi::get_one::<FsValue>(
                r#"select concat('{"type": "BYTES", "value": "', encode('helloworld'::bytea, 'base64'), '"}')::fsvalue"#
            ),
            Ok(Some(FsValue::Bytes(vec![
                0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x77, 0x6f, 0x72, 0x6c, 0x64
            ])))
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
                fs_number_from_integer(1),
                fs_null(),
                fs_boolean(true)
            ])))
        );
    }

    #[pg_test]
    fn test_fs_map() {
        let map = Spi::get_one::<FsValue>(
            r#"select '{
                "type": "MAP",
                "value": {
                    "foo": {"type": "NUMBER", "value": 1},
                    "bar": {"type": "NULL", "value": null},
                    "baz": {"type": "BOOLEAN", "value": true}
                }
            }'::fsvalue;"#,
        );

        assert_eq!(
            map,
            Ok(Some(FsValue::Map(BTreeMap::from([
                ("foo".to_owned(), fs_number_from_integer(1)),
                ("bar".to_owned(), fs_null()),
                ("baz".to_owned(), fs_boolean(true))
            ]))))
        );
    }

    #[pg_test]
    fn test_fs_map_get() {
        let map = FsValue::Map(BTreeMap::from([
            ("foo".to_owned(), fs_number_from_integer(1)),
            ("bar".to_owned(), fs_null()),
            ("baz".to_owned(), fs_boolean(true)),
            (
                "qux".to_owned(),
                FsValue::Map(BTreeMap::from([("foo".to_owned(), fs_null())])),
            ),
        ]));

        assert_eq!(
            fs_map_get(map.to_owned(), "foo"),
            Some(fs_number_from_integer(1))
        );
        assert_eq!(
            fs_map_get(fs_map_get(map.to_owned(), "qux").unwrap(), "foo"),
            Some(fs_null())
        );
        assert_eq!(fs_map_get(map.to_owned(), "quxx"), None);
    }

    #[pg_test]
    fn test_fs_le() {
        assert_eq!(fs_le(fs_null(), fs_boolean(true)), false);
        assert_eq!(fs_le(fs_null(), fs_number_from_integer(1)), false);
        assert_eq!(
            fs_le(fs_number_from_integer(0), fs_number_from_integer(0)),
            true
        );
        assert_eq!(
            fs_le(fs_number_from_integer(0), fs_number_from_double(0.1)),
            true
        );
        assert_eq!(
            fs_le(fs_number_from_integer(0), fs_number_from_integer(1)),
            true
        );
        assert_eq!(fs_le(fs_number_from_integer(1), fs_string("foo")), false);
    }

    #[pg_test]
    fn test_fs_ge() {
        assert_eq!(fs_ge(fs_null(), fs_boolean(true)), false);
        assert_eq!(fs_ge(fs_null(), fs_number_from_integer(1)), false);
        assert_eq!(
            fs_ge(fs_number_from_integer(0), fs_number_from_integer(-1)),
            true
        );
        assert_eq!(
            fs_ge(fs_number_from_integer(0), fs_number_from_integer(0)),
            true
        );
        assert_eq!(
            fs_ge(fs_number_from_integer(0), fs_number_from_integer(1)),
            false
        );
        assert_eq!(
            fs_ge(fs_number_from_integer(0), fs_number_from_double(0.1)),
            false
        );
        assert_eq!(fs_ge(fs_number_from_integer(1), fs_string("foo")), false);
    }

    #[pg_test]
    fn test_fs_neq() {
        assert_eq!(fs_neq(fs_null(), fs_null()), false);
        assert_eq!(fs_neq(fs_null(), fs_boolean(true)), true);
        assert_eq!(fs_neq(fs_null(), fs_number_from_integer(1)), true);
        assert_eq!(
            fs_neq(fs_number_from_integer(0), fs_number_from_integer(-1)),
            true
        );
        assert_eq!(
            fs_neq(fs_number_from_integer(0), fs_number_from_integer(0)),
            false
        );
        assert_eq!(
            fs_neq(fs_number_from_integer(0), fs_number_from_integer(1)),
            true
        );
        assert_eq!(
            fs_neq(fs_number_from_integer(0), fs_number_from_double(0.1)),
            true
        );
        assert_eq!(fs_neq(fs_number_from_integer(1), fs_string("foo")), true);
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
