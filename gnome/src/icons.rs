// SPDX-License-Identifier: GPL-3.0-or-later

//! Phosphor icon name handling (ui-spec §5, vault-spec §6.1).
//!
//! Icons are stored in the vault as bare Phosphor names (`flower-lotus`); the
//! app maps them to the bundled GTK symbolic icons (`ph-flower-lotus-symbolic`)
//! and shows a question mark when a name isn't shipped.

/// The GResource path registered as an icon theme location.
pub const RESOURCE_PATH: &str = "/io/github/stickgrinder/Quaderno/icons";

/// The fallback for a name we don't have (ui-spec §5).
pub const FALLBACK: &str = "ph-question-symbolic";

/// The GTK icon name for a stored Phosphor name, without checking that it
/// exists. The stored name is never rewritten.
pub fn icon_name(name: &str) -> String {
    format!("ph-{name}-symbolic")
}

/// The GTK icon name for a stored Phosphor name, falling back to
/// [`FALLBACK`] when the app doesn't ship that icon.
pub fn resolve(name: &str) -> String {
    let candidate = icon_name(name);
    if gtk::IconTheme::default().has_icon(&candidate) {
        candidate
    } else {
        FALLBACK.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_bare_names_to_symbolic_icons() {
        assert_eq!(icon_name("flower-lotus"), "ph-flower-lotus-symbolic");
    }

    #[test]
    fn fallback_is_a_shipped_question_mark() {
        assert_eq!(FALLBACK, "ph-question-symbolic");
    }
}
