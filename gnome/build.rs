// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiles the Blueprint UI files and bundles them into the GResource that the
//! app embeds. Keeping this in the Cargo build makes `cargo build` (and thus
//! `cargo clippy --workspace`) self-contained: it needs `blueprint-compiler`
//! and `glib-compile-resources`, both part of the GNOME SDK.

use std::path::{Path, PathBuf};
use std::process::Command;

const UI_FILE: &str = "quaderno-window.blp";
const GRESOURCE_FILE: &str = "quaderno.gresource.xml";
const GRESOURCE_TARGET: &str = "quaderno.gresource";

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let ui_dir = manifest_dir.join("ui");
    let data_dir = manifest_dir.join("data");

    println!(
        "cargo:rerun-if-changed={}",
        data_dir.join("styles/quaderno.xml").display()
    );
    compile_blueprint(&ui_dir, &out_dir);
    compile_resources(
        &[ui_dir.as_path(), data_dir.as_path(), out_dir.as_path()],
        &ui_dir,
    );
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
