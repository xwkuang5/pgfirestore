insert into fs_documents values (
    fs_reference('/users/1'), fs_null()
);
insert into fs_documents values (
    fs_reference('/users/1/posts/1'), fs_number_from_integer(1)
);
insert into fs_documents values (
    fs_reference('/posts/1'), fs_string('hello world')
);
-- This should fail because the document reference is not valid
insert into fs_documents values (
    fs_reference('/posts'), fs_string('hello world')
);
-- This should fail because the document reference already exists
insert into fs_documents values (
    fs_reference('/posts/1'), fs_string('hello world')
);

SELECT * FROM fs_collection(fs_reference('/users/1'), fs_string('posts'));