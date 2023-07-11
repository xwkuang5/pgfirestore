## pgfirestore

An exploration to build a [Firestore](https://firebase.google.com/docs/firestore) semantics query engine as a PostgreSQL extension.

### Dependencies

- [pgrx](https://github.com/pgcentralfoundation/pgrx): a framework that enables PostgreSQL extension development in Rust
- A Rust toolchain (`rustc` & `cargo`)
- PostgreSQL

### Getting Started

```bash
# Clone the repository
git clone https://github.com/xwkuang5/pgfirestore

# Build and run the extension and get dropped into a psql session
cargo pgrx run
```

```sql
psql (13.11)
Type "help" for help.

pgfirestore=# drop extension pgfirestore cascade; create extension pgfirestore;
DROP EXTENSION
CREATE EXTENSION

pgfirestore=# SELECT * FROM fs_documents;
                    reference                    |                                          properties
-------------------------------------------------+----------------------------------------------------------------------------------------------
 {"type":"REFERENCE","value":"/users/1"}         | {"type":"MAP","value":{"bar":{"type":"NUMBER","value":0},"foo":{"type":"NUMBER","value":0}}}
 {"type":"REFERENCE","value":"/users/1/posts/1"} | {"type":"MAP","value":{"bar":{"type":"NUMBER","value":1},"foo":{"type":"NUMBER","value":1}}}
 {"type":"REFERENCE","value":"/users/1/posts/2"} | {"type":"MAP","value":{"bar":{"type":"NUMBER","value":2},"foo":{"type":"NUMBER","value":2}}}
 {"type":"REFERENCE","value":"/users/2"}         | {"type":"MAP","value":{"foo":{"type":"NUMBER","value":2}}}
 {"type":"REFERENCE","value":"/users/3"}         | {"type":"MAP","value":{"foo":{"type":"NUMBER","value":3}}}
 {"type":"REFERENCE","value":"/users/4"}         | {"type":"MAP","value":{"foo":{"type":"NUMBER","value":4}}}
 {"type":"REFERENCE","value":"/users/5"}         | {"type":"MAP","value":{"foo":{"type":"NUMBER","value":5}}}
 {"type":"REFERENCE","value":"/posts/1"}         | {"type":"MAP","value":{"link":{"type":"REFERENCE","value":"/users/1/posts/1"}}}
 {"type":"REFERENCE","value":"/posts/2"}         | {"type":"MAP","value":{"link":{"type":"REFERENCE","value":"/users/1/posts/2"}}}
(9 rows)

pgfirestore=# SELECT * FROM fs_collection_group('posts');
                    reference                    |                                          properties
-------------------------------------------------+----------------------------------------------------------------------------------------------
 {"type":"REFERENCE","value":"/users/1/posts/1"} | {"type":"MAP","value":{"bar":{"type":"NUMBER","value":1},"foo":{"type":"NUMBER","value":1}}}
 {"type":"REFERENCE","value":"/users/1/posts/2"} | {"type":"MAP","value":{"bar":{"type":"NUMBER","value":2},"foo":{"type":"NUMBER","value":2}}}
 {"type":"REFERENCE","value":"/posts/1"}         | {"type":"MAP","value":{"link":{"type":"REFERENCE","value":"/users/1/posts/1"}}}
 {"type":"REFERENCE","value":"/posts/2"}         | {"type":"MAP","value":{"link":{"type":"REFERENCE","value":"/users/1/posts/2"}}}
(4 rows)

pgfirestore=# SELECT * FROM fs_collection_group('posts') WHERE properties->'foo' #>= fs_number_from_integer(2);
                    reference                    |                                          properties
-------------------------------------------------+----------------------------------------------------------------------------------------------
 {"type":"REFERENCE","value":"/users/1/posts/2"} | {"type":"MAP","value":{"bar":{"type":"NUMBER","value":2},"foo":{"type":"NUMBER","value":2}}}
(1 row)
```

Run `cargo pgrx schema` to list the set of SQL objects defined by the `pgfirestore` extension

### Data Types

pgfirestore extends PostgreSQL by defining a new `fsvalue` type supporting the same set of data types as [firestore](https://firebase.google.com/docs/firestore/manage-data/data-types) with the same type ordering.

### Representation

The in-memory and on-disk representation uses [Concise Binary Object Representation (CBOR)](https://datatracker.ietf.org/doc/html/rfc7049), which comes out of the box in `pgrx` by deriving the `serde::Serialize` and `serde::Deserialize` trait.

Any `PostgreSQL` custom data type can optionally implements an `input_function` and an `output_function` for converting from and to an external textual representation for use in SQL queries ([reference](https://www.postgresql.org/docs/current/sql-createtype.html)). `pgfirestore` uses JSON for textual representation of its data types with a custom schema:

```txt
 Null: {
  type: "NULL",
  value: null,
 }

 Boolean: {
  type: "BOOLEAN",
  value: true,
 }

 Number: {
  type: "NUMBER",
  value: 1,
 }

 Date: {
  type: "DATE",
  value: 1
 }

 String: {
  type: "STRING",
  value: "hello world"
 }

 Bytes: {
  type: "BYTES",
  value: "0x1234"
 }

 Reference: {
  type: "REFERENCE",
  value: "/users/1"
 }

 Geo point: {
  type: "GEOPOINT",
  value: [1.0, 2.0]
 }

 Array: {
  type: "ARRAY",
  value: [object]
 }

 Map: {
  type: "MAP",
  value: object
 }
```

### Custom Functions

- `fs_null`: constructs a SQL value with type `fsvalue` representing a Firestore NULL value
- `fs_nan`: constructs a SQL value with type `fsvalue` representing a Firestore NAN value
- `fs_boolean(bool)`: constructs a SQL value with type `fsvalue` representing a Firestore boolean value
- `fs_number_from_integer(integer)`: constructs a SQL value with type `fsvalue` representing a Firestore number value
  - `fs_number_from_double(double precision)`: constructs a SQL value with type `fsvalue` representing a Firestore number value
- `fs_string(text)`: constructs a SQL value with type `fsvalue` representing a Firestore string value
- `fs_reference(text)`: constructs a SQL value with type `fsvalue` representing a Firestore reference value
- `fs_array(ARRAY[fsvalue])`: constructs a SQL value with type `fsvalue` representing a Firestore array value
- `fs_map_from_entries(ARRAY[text], ARRAY[fsvalue])`: constructs a SQL value with type `fsvalue` representing a shallow Firestore map value

### Data Model

`pgfirestore` stores all data in a heap table named `fs_documents` with the following schema:

```sql
CREATE TABLE fs_documents (\n\
    reference fsvalue PRIMARY KEY,
    properties fsvalue
    CONSTRAINT valid_document_key CHECK (fs_is_valid_document_key(reference))
    CONSTRAINT valid_document_properties CHECK (fs_is_valid_document_properties(properties))
);
```

Since this is meant only as a simple query engine with no performance expectations, no secondary indexes are defined.

Firestore has a hierachical data model and supports structured queries on collection and collection groups. This is supported in `pgfirestore` using two custom table-valued functions:

- `fs_collection(parent fsvalue, collection_id text)`: returns a table consisting of all `collection_id` documents rooted under `parent`.
- `fs_collection_group(collection_id text)`: returns a table consisting of all `collection_id` documents rooted under the database root

### Custom Operators

The defailt comparison operators (`<`, `>`, `<=`, etc) on `fsvalue` implements Firestore type ordering with support for cross-type comparison. On the other hand, Firestore query operators (except for `!=`) compare only within type. To support this type of comparison, `pgfirestore` implements custom comparison operators `#<`, `#>`, `#<=`, `#>=`, `#=` and `#!=` with the same query semantics.

A document in Firestore is a map with arbitrary level of nesting. To retrieve a property of a document, `pgfirestore` supports a custom `->` operator.

### TODOs

In no particular order:

- Implement Firestore rules with triggers
- Fix mixed numerics comparison
- Implement `Date` and `GeoPoint` data type
- Support switching across different databases
- Fix various edge cases around `NaN`
- Investigate if there is a way in pgrx to declare a pg function that takes references of `fsvalue` instead of an owned value
- Fix misc method signature issues (borrow by reference where possible)
