#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Read a Quaderno sample vault using only `vault-spec/format.md`.

It decrypts with the `sqlcipher` command-line tool (the format's own cipher),
then reads the tables and prints them in the JSON export shape of the product
spec §10. It never decrypts to disk unencrypted (hard rule 1).

Usage:
    tools/read_sample.py [SAMPLE]                 # print the JSON
    tools/read_sample.py [SAMPLE] --check EXPECTED  # compare with a file
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import unicodedata
from pathlib import Path

# Tables in schema order, with the columns used to order rows (spec §4).
TABLES: list[tuple[str, str]] = [
    ("vault", "singleton"),
    ("entry", "id"),
    ("choice", "id"),
    ("subject", "id"),
    ("entry_choice", "entry_id, choice_id"),
    ("entry_subject", "entry_id, subject_id"),
    ("entry_color", "entry_id, color"),
    ("setting", "key"),
]

# The SQLCipher settings of spec §2.1, set explicitly.
CIPHER_PRAGMAS = [
    "PRAGMA cipher_page_size = 4096",
    "PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512",
    "PRAGMA kdf_iter = 256000",
    "PRAGMA cipher_hmac_algorithm = HMAC_SHA512",
    "PRAGMA cipher_plaintext_header_size = 0",
]

DEFAULT_PASSPHRASE = "Quaderno sample 1"
DEFAULT_SAMPLE = "fixtures/sample.quaderno"


def key_pragma(passphrase: str) -> str:
    """A `PRAGMA key` with the NFC passphrase (spec §2.2).

    The SQLCipher shell is only reachable through `PRAGMA key`, so this cannot
    use `sqlite3_key`; the passphrase is escaped as a SQL string.
    """
    normalized = unicodedata.normalize("NFC", passphrase)
    escaped = normalized.replace("'", "''")
    return "PRAGMA key = '%s'" % escaped


def parse_json_values(text: str) -> list[object]:
    """Parses every top-level JSON value, in order."""
    decoder = json.JSONDecoder()
    values: list[object] = []
    index = 0
    while index < len(text):
        while index < len(text) and text[index].isspace():
            index += 1
        if index >= len(text):
            break
        value, index = decoder.raw_decode(text, index)
        values.append(value)
    return values


def query(database: Path, passphrase: str, sql: str) -> object:
    command = ["sqlcipher", "-json"]
    command += ["-cmd", key_pragma(passphrase)]
    for pragma in CIPHER_PRAGMAS:
        command += ["-cmd", pragma]
    command += [str(database), sql]

    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr)
        raise SystemExit(f"sqlcipher failed for: {sql}")

    values = parse_json_values(completed.stdout)
    if not values:
        raise SystemExit(f"no JSON output for: {sql}")
    return values[-1]


def read_sample(database: Path, passphrase: str) -> dict[str, object]:
    result: dict[str, object] = {}
    for table, order in TABLES:
        result[table] = query(database, passphrase, f"SELECT * FROM {table} ORDER BY {order};")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sample", nargs="?", default=DEFAULT_SAMPLE, type=Path)
    parser.add_argument("--passphrase", default=DEFAULT_PASSPHRASE)
    parser.add_argument("--check", type=Path, help="expected JSON file to compare against")
    arguments = parser.parse_args()

    actual = read_sample(arguments.sample, arguments.passphrase)

    if arguments.check is None:
        json.dump(actual, sys.stdout, indent=2, ensure_ascii=False)
        sys.stdout.write("\n")
        return 0

    expected = json.loads(arguments.check.read_text(encoding="utf-8"))
    if actual == expected:
        print(f"{arguments.check} matches")
        return 0

    for table, _ in TABLES:
        if actual.get(table) != expected.get(table):
            print(f"table {table!r} differs", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
