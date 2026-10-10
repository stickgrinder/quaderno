#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the Rust binary with cargo and copies it where meson expects the
# custom_target output. Called by gnome/meson.build.
#
# Usage: cargo-build.sh CARGO MANIFEST TARGET_DIR OUTPUT

set -eu

cargo="$1"
manifest="$2"
target_dir="$3"
output="$4"

# `QUADERNO_CARGO_FLAGS` carries the SQLCipher feature selection from meson
# (empty for the default bundled build). It is intentionally word-split.
# shellcheck disable=SC2086
"$cargo" build --release --manifest-path "$manifest" -p quaderno \
    --target-dir "$target_dir" ${QUADERNO_CARGO_FLAGS:-}
cp "$target_dir/release/quaderno" "$output"
