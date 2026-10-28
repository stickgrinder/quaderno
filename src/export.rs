// SPDX-License-Identifier: GPL-3.0-or-later

//! Markdown export rendering (product spec §10): one file per entry, with YAML
//! front matter holding the metadata by readable name. The window gathers the
//! data from the vault and resolves labels through gettext; this module only
//! formats, so it is unit-tested directly.

use jiff::{Timestamp, Zoned};
use quaderno_vault::EntryType;
use quaderno_vault::time::format_dated;
use uuid::Uuid;

/// Everything needed to write one Markdown file, already resolved to display
/// strings.
#[derive(Debug, Clone)]
pub struct ExportEntry {
    /// The entry's id.
    pub id: Uuid,
    /// Journal page, dream or quick note.
    pub entry_type: EntryType,
    /// The date the entry is about.
    pub dated_at: Zoned,
    /// Creation time, UTC.
    pub created_at: Timestamp,
    /// Last edit time, UTC.
    pub updated_at: Timestamp,
    /// The quick note this entry came from.
    pub source_id: Option<Uuid>,
    /// The entry this is a conflict copy of.
    pub conflict_of: Option<Uuid>,
    /// Mood rating, 1–5.
    pub mood: Option<u8>,
    /// Energy rating, 1–5.
    pub energy: Option<u8>,
    /// Cognitive-load rating, 1–5.
    pub cognitive_load: Option<u8>,
    /// Sleep rating, 1–5.
    pub sleep: Option<u8>,
    /// Translated dream-flag names that are set.
    pub flags: Vec<String>,
    /// Translated emotion names.
    pub emotions: Vec<String>,
    /// Translated day-activity names.
    pub activities: Vec<String>,
    /// Translated colour names.
    pub colors: Vec<String>,
    /// People names.
    pub people: Vec<String>,
    /// Place names.
    pub places: Vec<String>,
    /// Thing names.
    pub things: Vec<String>,
    /// Tag names.
    pub tags: Vec<String>,
    /// The Markdown content.
    pub content: String,
}

/// The last eight hex characters of an id, the filename suffix (product spec
/// §10). The tail of a UUIDv7 is random, so entries made together don't collide.
pub fn id_suffix(id: Uuid) -> String {
    let simple = id.simple().to_string();
    simple[simple.len() - 8..].to_string()
}

/// The export filename for an entry: `YYYY-MM-DD_<type>_<id8>.md`.
pub fn file_name(entry: &ExportEntry) -> String {
    format!(
        "{}_{}_{}.md",
        entry.dated_at.strftime("%Y-%m-%d"),
        entry.entry_type.as_str(),
        id_suffix(entry.id),
    )
}

/// Renders the whole file: YAML front matter, then the content.
pub fn render(entry: &ExportEntry) -> String {
    format!("---\n{}---\n\n{}", front_matter(entry), entry.content)
}

/// Renders the YAML front matter block (ending with `---\n`).
pub fn front_matter(entry: &ExportEntry) -> String {
    let mut yaml = String::new();
    scalar(&mut yaml, "id", &entry.id.to_string());
    scalar(&mut yaml, "type", entry.entry_type.as_str());
    scalar(&mut yaml, "date", &format_dated(&entry.dated_at));
    scalar(&mut yaml, "created", &entry.created_at.to_string());
    scalar(&mut yaml, "updated", &entry.updated_at.to_string());
    if let Some(source) = entry.source_id {
        scalar(&mut yaml, "source", &source.to_string());
    }
    if let Some(conflict) = entry.conflict_of {
        scalar(&mut yaml, "conflict_of", &conflict.to_string());
    }
    rating(&mut yaml, "mood", entry.mood);
    rating(&mut yaml, "energy", entry.energy);
    rating(&mut yaml, "cognitive_load", entry.cognitive_load);
    rating(&mut yaml, "sleep", entry.sleep);
    sequence(&mut yaml, "flags", &entry.flags);
    sequence(&mut yaml, "emotions", &entry.emotions);
    sequence(&mut yaml, "activities", &entry.activities);
    sequence(&mut yaml, "colors", &entry.colors);
    sequence(&mut yaml, "people", &entry.people);
    sequence(&mut yaml, "places", &entry.places);
    sequence(&mut yaml, "things", &entry.things);
    sequence(&mut yaml, "tags", &entry.tags);
    yaml.push_str("---\n");
    yaml
}

fn scalar(yaml: &mut String, key: &str, value: &str) {
    yaml.push_str(key);
    yaml.push_str(": ");
    yaml.push_str(&quoted(value));
    yaml.push('\n');
}

fn rating(yaml: &mut String, key: &str, value: Option<u8>) {
    if let Some(value) = value {
        yaml.push_str(&format!("{key}: {value}\n"));
    }
}

fn sequence(yaml: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let items: Vec<String> = values.iter().map(|value| quoted(value)).collect();
    yaml.push_str(&format!("{key}: [{}]\n", items.join(", ")));
}

/// A double-quoted YAML scalar, so names with punctuation stay valid.
fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> ExportEntry {
        ExportEntry {
            id: Uuid::parse_str("01a0e54f-b000-7fb5-a336-339c06c3a42f").unwrap(),
            entry_type: EntryType::Dream,
            dated_at: quaderno_vault::time::parse_dated("2026-09-28T06:41:12+02:00").unwrap(),
            created_at: "2026-09-28T04:41:12Z".parse().unwrap(),
            updated_at: "2026-09-28T04:41:12Z".parse().unwrap(),
            source_id: None,
            conflict_of: None,
            mood: Some(4),
            energy: None,
            cognitive_load: None,
            sleep: None,
            flags: vec!["Lucid".into()],
            emotions: vec!["Calm".into()],
            activities: vec![],
            colors: vec!["Blue".into()],
            people: vec!["Marta".into()],
            places: vec![],
            things: vec![],
            tags: vec!["travel".into()],
            content: "# A dream\n\nBody text.".into(),
        }
    }

    #[test]
    fn filename_uses_the_date_type_and_random_id_tail() {
        assert_eq!(file_name(&entry()), "2026-09-28_dream_06c3a42f.md");
    }

    #[test]
    fn front_matter_has_readable_metadata_then_content() {
        let rendered = render(&entry());
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("type: \"dream\"\n"));
        assert!(rendered.contains("mood: 4\n"));
        assert!(rendered.contains("flags: [\"Lucid\"]\n"));
        assert!(rendered.contains("emotions: [\"Calm\"]\n"));
        assert!(rendered.contains("people: [\"Marta\"]\n"));
        // Empty sequences are omitted.
        assert!(!rendered.contains("places:"));
        // The content follows the front matter.
        assert!(rendered.ends_with("---\n\n# A dream\n\nBody text."));
    }

    #[test]
    fn names_with_punctuation_are_escaped() {
        let mut entry = entry();
        entry.tags = vec!["a: \"b\"\\c".into()];
        assert!(front_matter(&entry).contains(r#"tags: ["a: \"b\"\\c"]"#));
    }
}
