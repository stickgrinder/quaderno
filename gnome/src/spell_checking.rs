// SPDX-License-Identifier: GPL-3.0-or-later

//! libspelling integration for the editor (ui-spec §3.5).

use std::sync::Once;

static INIT: Once = Once::new();

/// Attaches a spell checker to `buffer` using the system dictionaries and
/// returns the adapter, which the caller must keep alive.
pub fn attach(buffer: &sourceview5::Buffer) -> libspelling::TextBufferAdapter {
    INIT.call_once(libspelling::init);
    let checker = libspelling::Checker::new(None, None);
    libspelling::TextBufferAdapter::new(buffer, &checker)
}
