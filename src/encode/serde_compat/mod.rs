//! Compatibility adapters for serde_json's private JSON shapes.

pub(super) mod private_struct_kind;
pub(super) mod serde_json_compat;

pub(super) use private_struct_kind::PrivateStructKind;
pub(super) use serde_json_compat::SerdeJsonCompat;
