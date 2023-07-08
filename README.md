## pgfirestore

An exploration to build a Firestore semantics query engine as a PostgreSQL extension.

## TODOs

In no particular order:

1. Implement Firestore rules with triggers
2. Implement Firestore array contains operators
3. Fix mixed numerics comparison
4. Fix various edge cases around `NaN`
5. Investigate if there is a way in pgrx to declare a pg function that takes references of `fsvalue` instead of an owned value
6. Fix misc method signature issues (borrow by reference where possible)

