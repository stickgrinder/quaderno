// SPDX-License-Identifier: GPL-3.0-or-later

//! Text rules from `vault-spec/format.md` §3: NFC normalization, the derived
//! title and the word count.

use unicode_normalization::UnicodeNormalization;

/// Returns `text` normalized to Unicode NFC.
pub fn nfc(text: &str) -> String {
    text.nfc().collect()
}

/// The title of an entry: the text of the first Markdown `#` heading,
/// otherwise the first non-blank line, otherwise `"Untitled"`.
///
/// The title is display-only; it is never stored (`vault-spec/format.md` §4.2).
pub fn title(content: &str) -> String {
    for line in content.lines() {
        if let Some(heading) = heading_text(line) {
            if heading.is_empty() {
                break;
            }
            return heading.to_string();
        }
    }

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    "Untitled".to_string()
}

/// Returns the text of `line` if it is an ATX Markdown heading (`# text`).
fn heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let after_hashes = trimmed.trim_start_matches('#');
    if after_hashes.len() == trimmed.len() {
        return None; // no leading '#'
    }
    if after_hashes.is_empty() {
        return Some("");
    }
    after_hashes
        .starts_with(char::is_whitespace)
        .then(|| after_hashes.trim())
}

/// The number of whitespace-separated words in `content`.
pub fn word_count(content: &str) -> usize {
    content.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_composes_decomposed_text() {
        // "è" as e + combining grave accent becomes the single code point.
        assert_eq!(nfc("e\u{0300}"), "è");
    }

    #[test]
    fn title_uses_the_first_heading() {
        assert_eq!(title("Some text\n# A heading\ntext"), "A heading");
        assert_eq!(title("### Three hashes"), "Three hashes");
    }

    #[test]
    fn title_falls_back_to_the_first_line() {
        assert_eq!(title("plain first line\nsecond"), "plain first line");
        assert_eq!(title("\n\nplain after blanks"), "plain after blanks");
    }

    #[test]
    fn title_without_hash_or_text_is_untitled() {
        assert_eq!(title(""), "Untitled");
        assert_eq!(title("   \n\t"), "Untitled");
    }

    #[test]
    fn hash_without_space_is_not_a_heading() {
        assert_eq!(title("#tag is not a heading"), "#tag is not a heading");
    }

    #[test]
    fn word_count_counts_whitespace_separated_words() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("one two  three\nfour"), 4);
    }
}
