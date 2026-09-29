// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared library for reading and writing Quaderno vaults.
//!
//! This is the only crate that touches a vault file: encryption, migrations,
//! typed data access, snapshots and validation, implementing
//! `vault-spec/format.md`. It contains no GTK and no Android code.

pub mod error;
pub mod migrations;
pub mod model;
pub mod text;
pub mod time;

mod collections;
mod crypto;
mod entries;
mod export;
mod links;
mod maintenance;
mod settings;
mod vault;

pub use error::VaultError;
pub use model::{
    Choice, ChoiceKind, Color, DreamFlag, Entry, EntryType, Rating, Subject, SubjectKind,
};
pub use vault::{Access, Vault};
