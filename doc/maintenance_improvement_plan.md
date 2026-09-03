# Maintenance Improvement Plan

## Execution order

1. Add contract tests for the optional `rs-ci` project hook, then implement it
   in local CI and the reusable workflow and document the contract.
2. Commit and synchronize `rs-ci`, return it to `dev-starfish`, and update the
   `rs-json/.rs-ci` submodule through `update-submodule.sh`.
3. Add failing tests for `JsonEncodeErrorSource`, implement and document the
   enum and `into_source`, then migrate `rs-config`, `rs-metadata`, and
   `rs-value` mappings.
4. Add a single crate-private `RawValue` compatibility token and migrate both
   internal users.
5. Add decimal-width, structured diagnostic, fuzz-corpus, and mutable-tree
   invariant coverage.
6. Replace the `rs-datatype` no-op accounting visitor with
   `JsonTreeReader::account`, backed by regression tests.
7. Add mutable traversal safety notes and complete the Rustdoc items.
8. Align the bilingual README, user guides, design documents, benchmark
   evidence, changelog, and migration guide.
9. Run `align-ci.sh` and `ci-check.sh` for every changed crate/repository.
10. Audit every design requirement and this plan against the resulting files;
    fix and rerun checks until no gap remains.
11. Group all current changes in every modified repository into topical English
    commits, push `dev-starfish`, fast-forward `dev` and `main`, push both, and
    finish on `dev-starfish` with clean working trees.

## Verification matrix

| Area | Authoritative evidence |
| --- | --- |
| CI hook | `rs-ci` unit tests, local script text, reusable workflow text |
| Public error API | compile tests, variant tests, downstream exhaustive matches |
| RawValue compatibility | one token definition and both imports |
| Numeric boundaries | explicit byte/value budget assertions for 9, 10, 99, 100, 255 |
| Fuzz invariants | checked-in corpus directories and fuzz workspace tests/build |
| Mutable traversal | safety docs plus configured Miri result |
| Documentation | bilingual links and semantic comparison |
| Per-crate CI | successful latest `align-ci.sh` and `ci-check.sh` output |
| Git delivery | equal branch commits, successful pushes, clean final status |
