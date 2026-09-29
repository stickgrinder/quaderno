// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared library for reading and writing Quaderno vaults.
//!
//! This is the only crate that touches a vault file: encryption, migrations,
//! typed data access, snapshots and validation, implementing
//! `vault-spec/format.md`. It contains no GTK and no Android code.
//!
//! Added as a stub in milestone M0 (repository scaffolding); the vault logic
//! arrives in M1.

#![forbid(unsafe_code)]
