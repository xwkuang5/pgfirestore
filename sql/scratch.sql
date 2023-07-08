-- Display `NULL` as `<null>`
\ pset null '<null>'
insert into
    fs_documents
values
    (
        fs_reference('/posts/1'),
        fs_map_from_entries(ARRAY ['foo'], ARRAY [fs_string('bar')])
    );

-- This should fail because the document reference is not valid
insert into
    fs_documents
values
    (
        fs_reference('/posts'),
        fs_map_from_entries(ARRAY ['foo'], ARRAY [fs_string('bar')])
    );

-- This should fail because the document reference already exists
insert into
    fs_documents
values
    (
        fs_reference('/posts/1'),
        fs_map_from_entries(ARRAY ['foo'], ARRAY [fs_string('bar')])
    );

-- This should fail because the document properties is not valid
insert into
    fs_documents
values
    (
        fs_reference('/posts/2'),
        fs_string('hello world')
    );

-- An example collection query against the root
select
    *
from
    fs_collection(fs_reference('/'), 'posts');

-- An example recursive field value retrieval
with base as (
    select
        '{
            "type": "MAP",
            "value": {
                "foo": {"type": "NUMBER", "value": 1},
                "bar": {"type": "NULL", "value": null},
                "baz": {"type": "BOOLEAN", "value": true},
                "qux": {
                    "type": "MAP",
                    "value": {
                        "foo": {"type": "NUMBER", "value": 1}
                    }
                }
            }
        }' :: fsvalue as val
)
select
    (val -> 'qux') -> 'foo'
from
    base;

with base as (
    select
        fs_array(
            ARRAY [
            fs_number_from_integer(1),
            fs_number_from_integer(2)]
        ) as val
)
select
    fs_array_contains(val, fs_number_from_integer(0)) as contain_0,
    fs_array_contains(val, fs_number_from_integer(1)) as contain_1,
    fs_array_contains_any(
        val,
        ARRAY [fs_number_from_integer(1), fs_number_from_integer(0)]
    ) as contain_0_or_1
from
    base;

-- Example showing sizes (in-memory) of CBOR vs other representation
-- pg_column_size | pg_column_size | pg_column_size | pg_column_size
-- ---------------+----------------+----------------+----------------
--             24 |             28 |             28 |             33
select
    pg_column_size(fs_value_string('hello world')),
    pg_column_size('{"String":"hello world"}' :: text),
    pg_column_size('{"String":"hello world"}' :: json),
    pg_column_size('{"String":"hello world"}' :: jsonb);