// SPDX-License-Identifier: GPL-3.0-or-later

//! Creates `fixtures/sample.quaderno` and its expected `fixtures/sample.json`
//! (vault spec §10). Run with:
//!
//! ```sh
//! cargo run -p quaderno-vault --example generate_fixtures
//! ```
//!
//! The sample is the reference material other clients test against; it is
//! encrypted with the passphrase `Quaderno sample 1`.

use std::path::PathBuf;

use quaderno_vault::{ChoiceKind, Color, EntryType, SubjectKind, Vault};

const PASSPHRASE: &str = "Quaderno sample 1";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = std::env::var_os("QUADERNO_FIXTURES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("fixtures")
        });
    std::fs::create_dir_all(&fixtures)?;

    let vault_path = fixtures.join("sample.quaderno");
    let json_path = fixtures.join("sample.json");
    for path in [&vault_path, &json_path] {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }

    let mut vault = Vault::create(&vault_path, PASSPHRASE)?;
    populate(&mut vault)?;
    std::fs::write(&json_path, format!("{}\n", vault.export_json()?))?;

    println!("wrote {}", vault_path.display());
    println!("wrote {}", json_path.display());
    Ok(())
}

fn populate(vault: &mut Vault) -> Result<(), Box<dyn std::error::Error>> {
    vault.set_day_end((3, 0))?;

    let emotion = |vault: &Vault, label: &str| -> uuid::Uuid {
        vault
            .choices(ChoiceKind::Emotion, true)
            .unwrap()
            .into_iter()
            .find(|choice| choice.display_label() == label)
            .unwrap()
            .id
    };
    let activity = |vault: &Vault, label: &str| -> uuid::Uuid {
        vault
            .choices(ChoiceKind::Activity, true)
            .unwrap()
            .into_iter()
            .find(|choice| choice.display_label() == label)
            .unwrap()
            .id
    };

    let marta = vault.create_subject(SubjectKind::Person, "Marta")?;
    let lecco = vault.create_subject(SubjectKind::Place, "Lecco")?;
    let camera = vault.create_subject(SubjectKind::Thing, "Film camera")?;
    let weekend = vault.create_subject(SubjectKind::Tag, "weekend")?;
    let lake = vault.create_subject(SubjectKind::Tag, "lake")?;

    let saturday = vault.create_entry(
        EntryType::Journal,
        "# A slow Saturday by the lake\n\nWoke up late for once: **nine hours** of sleep \
         and no alarm.\n\n- The light on the mountains around four\n- A quiet first hour",
        &dated("2026-09-26T21:14:00+02:00"),
    )?;
    vault.set_entry_rating(saturday, quaderno_vault::Rating::Mood, Some(4))?;
    vault.set_entry_rating(saturday, quaderno_vault::Rating::Energy, Some(4))?;
    vault.set_entry_rating(saturday, quaderno_vault::Rating::CognitiveLoad, Some(3))?;
    vault.set_entry_rating(saturday, quaderno_vault::Rating::Sleep, Some(4))?;
    for label in ["Calm", "Gratitude"] {
        vault.link_choice(saturday, emotion(vault, label))?;
    }
    for label in ["Walk", "Travel", "Friends & family"] {
        vault.link_choice(saturday, activity(vault, label))?;
    }
    for color in [Color::Green, Color::Blue] {
        vault.link_color(saturday, color)?;
    }
    for subject in [marta, lecco, camera, weekend, lake] {
        vault.link_subject(saturday, subject)?;
    }

    let dream = vault.create_entry(
        EntryType::Dream,
        "# The train that became a boat\n\nThe carriage kept filling with water, slowly, \
         like a bath.",
        &dated("2026-09-22T07:40:00+02:00"),
    )?;
    vault.set_dream_flag(dream, quaderno_vault::DreamFlag::Lucid, Some(true))?;
    vault.set_dream_flag(dream, quaderno_vault::DreamFlag::Recurring, Some(true))?;
    vault.set_entry_rating(dream, quaderno_vault::Rating::Mood, Some(4))?;
    vault.set_entry_rating(dream, quaderno_vault::Rating::Sleep, Some(4))?;
    vault.link_choice(dream, emotion(vault, "Calm"))?;
    vault.link_color(dream, Color::Teal)?;

    let note = vault.create_entry(
        EntryType::Note,
        "Call Dad, then book the train for Saturday",
        &dated("2026-09-28T09:12:00+02:00"),
    )?;
    vault.link_subject(note, weekend)?;

    // One note expanded into a journal page, to exercise `source_id`.
    vault.expand_note(note, EntryType::Journal)?;

    Ok(())
}

fn dated(value: &str) -> jiff::Zoned {
    quaderno_vault::time::parse_dated(value).expect("the fixture dates are valid")
}
