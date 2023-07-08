## pgfirestore

An exploration to build a Firestore semantics query engine as a PostgreSQL extension.

## TODOs

In no particular order:

- Implement Firestore rules with triggers
- Fix mixed numerics comparison
- Fix various edge cases around `NaN`
- Investigate if there is a way in pgrx to declare a pg function that takes references of `fsvalue` instead of an owned value
- Fix misc method signature issues (borrow by reference where possible)
