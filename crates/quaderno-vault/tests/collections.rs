// SPDX-License-Identifier: GPL-3.0-or-later

//! Choices, subjects, links, notes and settings.

use quaderno_vault::{ChoiceKind, Color, EntryType, SubjectKind, Vault, VaultError};
use tempfile::TempDir;

fn new_vault() -> (TempDir, Vault) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.quaderno");
    let vault = Vault::create(&path, "passphrase").unwrap();
    (dir, vault)
}

fn dated(value: &str) -> jiff::Zoned {
    quaderno_vault::time::parse_dated(value).unwrap()
}

fn journal(vault: &mut Vault) -> uuid::Uuid {
    vault
        .create_entry(
            EntryType::Journal,
            "day",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap()
}

#[test]
fn defaults_are_seeded_and_can_be_renamed_and_reset() {
    let (_dir, mut vault) = new_vault();
    let emotions = vault.choices(ChoiceKind::Emotion, true).unwrap();
    assert_eq!(emotions.len(), 12);
    let calm = emotions
        .iter()
        .find(|choice| choice.display_label() == "Calm")
        .unwrap()
        .id;

    vault.rename_choice(calm, "Sereno").unwrap();
    assert_eq!(vault.choice(calm).unwrap().display_label(), "Sereno");
    vault.reset_choice_label(calm).unwrap();
    assert_eq!(vault.choice(calm).unwrap().display_label(), "Calm");
}

#[test]
fn user_choices_cannot_be_reset() {
    let (_dir, mut vault) = new_vault();
    let id = vault
        .create_choice(ChoiceKind::Activity, "Gardening", "plant")
        .unwrap();
    assert_eq!(vault.choice(id).unwrap().display_label(), "Gardening");
    assert!(matches!(
        vault.reset_choice_label(id),
        Err(VaultError::Invalid(_))
    ));
}

#[test]
fn hidden_choices_are_filtered() {
    let (_dir, mut vault) = new_vault();
    let id = vault
        .create_choice(ChoiceKind::Emotion, "Ennui", "cloud")
        .unwrap();
    vault.set_choice_hidden(id, true).unwrap();
    assert!(
        vault
            .choices(ChoiceKind::Emotion, false)
            .unwrap()
            .iter()
            .all(|c| c.id != id)
    );
    assert!(
        vault
            .choices(ChoiceKind::Emotion, true)
            .unwrap()
            .iter()
            .any(|c| c.id == id)
    );
}

#[test]
fn choice_links_and_type_rule() {
    let (_dir, mut vault) = new_vault();
    let entry = journal(&mut vault);
    let choice = vault
        .create_choice(ChoiceKind::Emotion, "Calm", "flower-lotus")
        .unwrap();

    vault.link_choice(entry, choice).unwrap();
    assert_eq!(vault.entry_choices(entry).unwrap().len(), 1);
    vault.unlink_choice(entry, choice).unwrap();
    assert!(vault.entry_choices(entry).unwrap().is_empty());
    vault.link_choice(entry, choice).unwrap();
    assert_eq!(vault.entry_choices(entry).unwrap().len(), 1);

    let note = vault
        .create_entry(
            EntryType::Note,
            "quick",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    let error = vault.link_choice(note, choice).unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn deleting_a_used_choice_is_refused() {
    let (_dir, mut vault) = new_vault();
    let entry = journal(&mut vault);
    let choice = vault
        .create_choice(ChoiceKind::Emotion, "Calm", "flower-lotus")
        .unwrap();
    vault.link_choice(entry, choice).unwrap();
    assert!(matches!(
        vault.soft_delete_choice(choice),
        Err(VaultError::Invalid(_))
    ));
    assert_eq!(vault.choice_usage(choice).unwrap(), 1);
}

#[test]
fn subjects_rename_merge_and_delete() {
    let (_dir, mut vault) = new_vault();
    let entry = journal(&mut vault);

    let giulia = vault.create_subject(SubjectKind::Person, "Giulia").unwrap();
    let giorgio = vault
        .create_subject(SubjectKind::Person, "Giorgio")
        .unwrap();
    vault.link_subject(entry, giorgio).unwrap();

    vault.rename_subject(giulia, "Giulia B.").unwrap();
    assert_eq!(vault.subject(giulia).unwrap().name, "Giulia B.");

    vault.merge_subjects(giulia, giorgio).unwrap();
    assert!(matches!(vault.subject(giorgio), Err(VaultError::NotFound)));
    let people = vault
        .entry_subjects(entry, Some(SubjectKind::Person))
        .unwrap();
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].id, giulia);

    assert!(matches!(
        vault.soft_delete_subject(giulia),
        Err(VaultError::Invalid(_))
    ));
}

#[test]
fn note_tags_are_allowed_but_people_are_not() {
    let (_dir, mut vault) = new_vault();
    let note = vault
        .create_entry(
            EntryType::Note,
            "quick",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    let tag = vault.create_subject(SubjectKind::Tag, "weekend").unwrap();
    vault.link_subject(note, tag).unwrap();
    assert_eq!(
        vault
            .entry_subjects(note, Some(SubjectKind::Tag))
            .unwrap()
            .len(),
        1
    );

    let person = vault.create_subject(SubjectKind::Person, "Marta").unwrap();
    let error = vault.link_subject(note, person).unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn colours_link_and_unlink() {
    let (_dir, mut vault) = new_vault();
    let entry = journal(&mut vault);
    vault.link_color(entry, Color::Green).unwrap();
    vault.link_color(entry, Color::Blue).unwrap();
    let mut colors = vault.entry_colors(entry).unwrap();
    colors.sort_by_key(|color| color.as_str());
    assert_eq!(colors.len(), 2);
    vault.unlink_color(entry, Color::Green).unwrap();
    assert_eq!(vault.entry_colors(entry).unwrap(), vec![Color::Blue]);
}

#[test]
fn expand_a_note_copies_text_and_tags() {
    let (_dir, mut vault) = new_vault();
    let note = vault
        .create_entry(EntryType::Note, "idea", &dated("2026-09-27T09:12:00+02:00"))
        .unwrap();
    let tag = vault.create_subject(SubjectKind::Tag, "ideas").unwrap();
    vault.link_subject(note, tag).unwrap();

    let expanded = vault.expand_note(note, EntryType::Journal).unwrap();
    let new = vault.entry(expanded).unwrap();
    assert_eq!(new.entry_type, EntryType::Journal);
    assert_eq!(new.content, "idea");
    assert_eq!(new.source_id, Some(note));
    assert_eq!(vault.entry_subjects(expanded, None).unwrap().len(), 1);
    assert_eq!(vault.expanded_entries(note).unwrap(), vec![expanded]);

    // The note is unchanged.
    assert_eq!(vault.entry(note).unwrap().entry_type, EntryType::Note);

    assert!(matches!(
        vault.expand_note(note, EntryType::Note),
        Err(VaultError::Invalid(_))
    ));
}

#[test]
fn day_end_defaults_and_updates() {
    let (_dir, mut vault) = new_vault();
    assert_eq!(vault.day_end().unwrap(), (3, 0));
    vault.set_day_end((5, 30)).unwrap();
    assert_eq!(vault.day_end().unwrap(), (5, 30));
    assert!(matches!(
        vault.set_day_end((7, 0)),
        Err(VaultError::Invalid(_))
    ));
}
