insert into
    fs_documents
values
    (fs_reference('/users/1'), fs_null());

insert into
    fs_documents
values
    (
        fs_reference('/users/1/posts/1'),
        fs_number_from_integer(1)
    );

insert into
    fs_documents
values
    (
        fs_reference('/posts/1'),
        fs_string('hello world')
    );

-- This should fail because the document reference is not valid
insert into
    fs_documents
values
    (
        fs_reference('/posts'),
        fs_string('hello world')
    );

-- This should fail because the document reference already exists
insert into
    fs_documents
values
    (
        fs_reference('/posts/1'),
        fs_string('hello world')
    );

-- An example collection query
select
    *
from
    fs_collection(fs_reference('/users/1'), 'posts');

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
    (val->'qux')->'foo'
from
    base;