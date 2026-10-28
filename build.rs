// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiles the Blueprint UI files and bundles them into the GResource that the
//! app embeds. Keeping this in the Cargo build makes `cargo build` (and thus
//! `cargo clippy --workspace`) self-contained: it needs `blueprint-compiler`
//! and `glib-compile-resources`, both part of the GNOME SDK.
//!
//! It also converts the vendored Phosphor Icons 2.1 SVGs into GTK symbolic
//! icons named `ph-<name>-symbolic` (ui-spec §5) and bundles those too.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const UI_FILE: &str = "quaderno-window.blp";
const GRESOURCE_FILE: &str = "quaderno.gresource.xml";
const GRESOURCE_TARGET: &str = "quaderno.gresource";
const ICONS_GRESOURCE_TARGET: &str = "quaderno-icons.gresource";

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let ui_dir = manifest_dir.join("ui");
    let data_dir = manifest_dir.join("data");

    println!(
        "cargo:rerun-if-changed={}",
        data_dir.join("quaderno.css").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        data_dir.join("styles/quaderno.xml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        data_dir.join("styles/quaderno-dark.xml").display()
    );
    compile_blueprint(&ui_dir, &out_dir);
    compile_resources(
        &[ui_dir.as_path(), data_dir.as_path(), out_dir.as_path()],
        &ui_dir,
    );
    compile_icons(&data_dir, &out_dir);
}

fn compile_blueprint(ui_dir: &Path, out_dir: &Path) {
    let blueprint = ui_dir.join(UI_FILE);
    println!("cargo:rerun-if-changed={}", blueprint.display());

    let status = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(out_dir)
        .arg(ui_dir)
        .arg(&blueprint)
        .status()
        .expect("blueprint-compiler must be installed to build the UI");
    assert!(
        status.success(),
        "blueprint-compiler failed to compile {UI_FILE}"
    );
}

fn compile_resources(source_dirs: &[&Path], ui_dir: &Path) {
    let gresource = ui_dir.join(GRESOURCE_FILE);
    glib_build_tools::compile_resources(
        source_dirs,
        gresource
            .to_str()
            .expect("the UI directory path is valid UTF-8"),
        GRESOURCE_TARGET,
    );
}

/// Converts the vendored Phosphor SVGs into `ph-<name>-symbolic.svg` resources.
///
/// Phosphor's regular weight fills its shapes with `currentColor`; GTK's
/// symbolic recolouring expects a plain fill (Adwaita uses `#2e3436`), so the
/// conversion replaces the keyword and drops the dependency on `currentColor`.
fn compile_icons(data_dir: &Path, out_dir: &Path) {
    let regular = data_dir.join("icons/phosphor/regular");
    println!("cargo:rerun-if-changed={}", regular.display());

    let converted = out_dir.join("phosphor");
    fs::create_dir_all(&converted).expect("the icon output directory");

    let mut names = Vec::new();
    let entries = fs::read_dir(&regular).expect("the vendored Phosphor directory");
    for entry in entries {
        let path = entry.expect("a readable Phosphor entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("svg") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let svg = fs::read_to_string(&path).expect("a readable Phosphor SVG");
        let symbolic = svg.replace("currentColor", "#2e3436");
        let name = format!("ph-{stem}-symbolic.svg");
        fs::write(converted.join(&name), symbolic).expect("a writable icon");
        names.push(name);
    }
    names.sort();
    assert!(
        !names.is_empty(),
        "no Phosphor icons found in {}",
        regular.display()
    );

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n\
         \x20 <gresource prefix=\"/io/github/stickgrinder/Quaderno/icons/scalable/actions\">\n",
    );
    for name in &names {
        xml.push_str("    <file>");
        xml.push_str(name);
        xml.push_str("</file>\n");
    }
    xml.push_str("  </gresource>\n</gresources>\n");

    let xml_path = out_dir.join("quaderno-icons.gresource.xml");
    fs::write(&xml_path, xml).expect("the generated icon manifest");
    glib_build_tools::compile_resources(
        &[converted.as_path()],
        xml_path
            .to_str()
            .expect("the icon output path is valid UTF-8"),
        ICONS_GRESOURCE_TARGET,
    );
}
