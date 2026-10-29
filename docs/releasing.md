# Releasing Quaderno

Quaderno uses trunk-based development: `main` is always green and
release-ready, and releases are annotated tags on `main`.

## Prepare the release

1. Choose the version, for example `0.1.0`, and update it in:
   - `Cargo.toml` (`[workspace.package] version` and `Cargo.lock` via
     `cargo check`);
   - `meson.build` (`project(... version: ...)`);
   - `data/io.github.stickgrinder.Quaderno.metainfo.xml` (the `<release>`
     version and date);
   - `build-aux/packaging/nfpm-deb.yaml` and `nfpm-rpm.yaml` (`version`);
   - `build-aux/aur/PKGBUILD` (`pkgver`) and `build-aux/aur/quaderno-git`
     needs no version change.
2. Update `CHANGELOG.md`.
3. Run the full checks:

   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   meson setup build . && meson compile -C build && meson test -C build
   flatpak-builder --user --install --force-clean build-flatpak \
     build-aux/io.github.stickgrinder.Quaderno.json
   python3 tools/read_sample.py fixtures/sample.quaderno --check fixtures/sample.json
   tools/check-release-version vX.Y.Z
   ```

4. Commit and merge to `main` through a pull request. Do not tag a commit that
   is not on `main`.

## Tag and publish

```sh
git tag -a vX.Y.Z -m "Quaderno X.Y.Z"
git push origin vX.Y.Z
```

The tag triggers `.github/workflows/release.yml`, which verifies the tag
against every version string and then builds:

- `quaderno-X.Y.Z-x86_64.flatpak`
- `quaderno_X.Y.Z-1_amd64_debian_trixie.deb` (Debian 13)
- `quaderno_X.Y.Z-1_amd64_ubuntu_resolute.deb` (Ubuntu 26.04 LTS)
- `quaderno-X.Y.Z-1.x86_64.rpm` (current Fedora)
- `quaderno-X.Y.Z-x86_64.tar.xz`
- `SHA256SUMS`

It creates the GitHub Release with the `CHANGELOG.md` as its body. Verify the
assets and the release page once the workflow finishes.

The workflow can also be started by hand with a tag input, for a rerun.

## Patch releases

For a fix after a release:

1. Create `release/X.Y` from the release tag.
2. Cherry-pick the fix (or commit it there), bump the patch version and the
   changelog.
3. Tag `vX.Y.Z` and push; the release workflow publishes it.
4. Merge the fix back into `main`.

## AUR (separate repository)

The templates in `build-aux/aur/` are the source of truth for the recipes; the
AUR package lives in its own `ssh://aur@aur.archlinux.org/quaderno.git`
repository.

For a release:

1. Copy `build-aux/aur/PKGBUILD` into the AUR clone.
2. Check that `pkgver` and the `source` tag (`v$pkgver`) are right.
3. Run `updpkgsums` to fill `sha256sums` (the template ships `SKIP`).
4. Generate the metadata: `makepkg --printsrcinfo > .SRCINFO`.
5. Test with `makepkg -si` (ideally in a clean chroot with `devtools`).
6. Commit and push the AUR repository.

`quaderno-git` tracks the repository and needs no version bump.

## Flathub

Flathub publication is planned after the first GitHub releases. The manifest
is already at `build-aux/io.github.stickgrinder.Quaderno.json`; the AppStream
metadata, screenshots and license are in place.
