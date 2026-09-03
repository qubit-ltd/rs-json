# Changelog

[简体中文](CHANGELOG.zh_CN.md)

All notable changes to `qubit-json` are recorded here. The project follows
semantic versioning for published releases.

## Unreleased - 0.8.0

- Reorganized the crate into explicit `decode`, `encode`, and `value` domains.
- Added strict and normalizing decoders backed by resource-accounting sessions.
- Added budgeted strict encoding, materialized-value construction, duplicate-key
  rejection, and iterative read/mutation APIs.
- Defined the signed-`i64`, unsigned-`u64`, and finite-`f64` number contract.
- Added structured, privacy-aware decode and encode error models, including the
  owned `JsonEncodeErrorSource` mapping API.
- Added fuzz, Miri, documentation-example, compatibility, and benchmark suites.

This version is under development and has not been published to crates.io.
See the [0.3 to 0.8 migration guide](doc/migration_0_3_to_0_8.md).

## 0.7.0 - 2026

- Hardened the original lenient decoder with UTF-8 entry points, normalized
  input limits, and redacted-by-default diagnostics.

## 0.6.0 - 2026

- Introduced builder-based decoder configuration and broader resource limits.

## 0.3.6 - 2026

- Final patch release of the original root-level `LenientJsonDecoder` API.

