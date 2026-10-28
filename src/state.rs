// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK-free state for the welcome/unlock flow, so it can be unit-tested.
//! Behavior is from `docs/product-spec.md` §3.1–3.2.

use std::time::Duration;

/// Which page of the window stack is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No vault configured yet.
    Welcome,
    /// A vault is configured and locked.
    Unlock,
    /// The configured vault is missing or invalid.
    Error,
    /// A vault is open.
    Main,
}

/// Why the creation form cannot be submitted yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateValidity {
    /// Both fields are still empty.
    Empty,
    /// Fewer than 8 characters.
    TooShort,
    /// The two passphrases differ.
    Mismatch,
    /// Ready to create.
    Ready,
}

/// The minimum passphrase length (spec §3.1).
pub const MIN_PASSPHRASE_CHARS: usize = 8;

/// The state of the "Create a new journal" form.
#[derive(Debug, Clone, Default)]
pub struct CreateForm {
    /// The passphrase field.
    pub passphrase: String,
    /// The repeat field.
    pub repeat: String,
    /// Whether the "I understand…" box is ticked.
    pub confirmed: bool,
}

impl CreateForm {
    /// Checks the form against the creation rules.
    pub fn validity(&self) -> CreateValidity {
        if self.passphrase.is_empty() && self.repeat.is_empty() {
            return CreateValidity::Empty;
        }
        if self.passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
            return CreateValidity::TooShort;
        }
        if self.passphrase != self.repeat {
            return CreateValidity::Mismatch;
        }
        CreateValidity::Ready
    }

    /// Whether the Create button is enabled.
    pub fn can_create(&self) -> bool {
        self.confirmed && self.validity() == CreateValidity::Ready
    }

    /// An informative strength hint in `0.0..=1.0` (not blocking).
    pub fn strength(&self) -> f64 {
        strength(&self.passphrase)
    }
}

/// A small, informative passphrase strength estimate.
pub fn strength(passphrase: &str) -> f64 {
    let length = passphrase.chars().count();
    if length == 0 {
        return 0.0;
    }
    let classes = [
        passphrase.chars().any(|c| c.is_lowercase()),
        passphrase.chars().any(|c| c.is_uppercase()),
        passphrase.chars().any(|c| c.is_ascii_digit()),
        passphrase.chars().any(|c| !c.is_alphanumeric()),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();

    let length_score = (length as f64 / 20.0).min(1.0);
    let variety_score = classes as f64 / 4.0;
    (0.6 * length_score + 0.4 * variety_score).clamp(0.05, 1.0)
}

/// Limits how often a wrong passphrase may be tried (spec §3.2): after
/// [`MAX_FAILURES`] wrong attempts, the next attempt waits [`DELAY`].
#[derive(Debug, Clone)]
pub struct Throttle {
    failures: u32,
    blocked_until: Option<Duration>,
}

/// Wrong attempts before the delay.
pub const MAX_FAILURES: u32 = 5;
/// The delay after too many wrong attempts.
pub const DELAY: Duration = Duration::from_secs(30);

impl Default for Throttle {
    fn default() -> Self {
        Self::new()
    }
}

impl Throttle {
    /// A fresh throttle.
    pub fn new() -> Self {
        Self {
            failures: 0,
            blocked_until: None,
        }
    }

    /// Whether an unlock attempt is allowed at `now` (a monotonic time).
    pub fn is_allowed(&self, now: Duration) -> bool {
        self.blocked_until.is_none_or(|until| now >= until)
    }

    /// Records a wrong attempt at `now` and returns whether it starts a delay.
    pub fn record_failure(&mut self, now: Duration) -> bool {
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            self.failures = 0;
            self.blocked_until = Some(now + DELAY);
            true
        } else {
            false
        }
    }

    /// Clears the counter after a correct passphrase.
    pub fn reset(&mut self) {
        self.failures = 0;
        self.blocked_until = None;
    }

    /// The time left before the next attempt, if blocked.
    pub fn remaining(&self, now: Duration) -> Option<Duration> {
        self.blocked_until
            .filter(|until| now < *until)
            .map(|until| until - now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn create_form_requires_two_matching_passphrases_of_eight_chars() {
        let mut form = CreateForm::default();
        assert_eq!(form.validity(), CreateValidity::Empty);

        form.passphrase = "short".into();
        assert_eq!(form.validity(), CreateValidity::TooShort);

        form.passphrase = "long enough".into();
        form.repeat = "long enough".into();
        assert_eq!(form.validity(), CreateValidity::Ready);

        form.repeat = "different".into();
        assert_eq!(form.validity(), CreateValidity::Mismatch);
    }

    #[test]
    fn create_requires_the_confirmation_box() {
        let mut form = CreateForm {
            passphrase: "long enough".into(),
            repeat: "long enough".into(),
            confirmed: false,
        };
        assert!(!form.can_create());
        form.confirmed = true;
        assert!(form.can_create());
    }

    #[test]
    fn strength_grows_with_length_and_variety() {
        assert_eq!(strength(""), 0.0);
        assert!(strength("aaaaaaaa") < strength("aA1!aA1!aA1!aA1!"));
        assert!(strength("aaaa") < strength("aaaaaaaaaaaa"));
    }

    #[test]
    fn throttle_blocks_after_five_failures_for_thirty_seconds() {
        let mut throttle = Throttle::new();
        for failure in 0..(MAX_FAILURES - 1) {
            assert!(!throttle.record_failure(at(failure as u64)));
            assert!(throttle.is_allowed(at(1000)));
        }
        // The fifth failure starts the delay.
        assert!(throttle.record_failure(at(0)));
        assert!(!throttle.is_allowed(at(29)));
        assert_eq!(throttle.remaining(at(10)), Some(Duration::from_secs(20)));
        assert!(throttle.is_allowed(at(30)));
        assert_eq!(throttle.remaining(at(30)), None);
    }

    #[test]
    fn throttle_resets_after_success() {
        let mut throttle = Throttle::new();
        throttle.record_failure(at(0));
        throttle.reset();
        assert!(throttle.is_allowed(at(0)));
    }
}
