insert into fs_documents values (
    fs_reference('projects/test-project/databases/test-database/documents/users/1'), fs_null()
);
insert into fs_documents values (
    fs_reference('projects/test-project/databases/test-database/documents/posts/1'), fs_string('hello world')
);
-- This should fail because the document reference is not valid
insert into fs_documents values (
    fs_reference('projects/test-project/databases/test-database/documents/posts'), fs_string('hello world')
);