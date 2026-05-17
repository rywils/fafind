# Release process

## 1. Tag and push

Update the version in `Cargo.toml` and `Cargo.lock` (`cargo build --release`), commit, then:

```sh
git tag v1.1.0
git push origin v1.1.0
```

Tagging is the only step required to build and publish binaries.
Updating AUR and Homebrew formulas is still manual.
---

## 2. What the GitHub Action does

On every `v*` tag push, `.github/workflows/release.yml`:

1. Builds release binaries for three targets in parallel:
   - `x86_64-unknown-linux-gnu` (native, Ubuntu runner)
   - `aarch64-unknown-linux-gnu` (via `cross`, Ubuntu runner)
   - `aarch64-apple-darwin` (native, macOS 14 runner)
2. Packages each Linux/macOS binary as `fafind-<platform>-<tag>.tar.gz` (e.g. `fafind-linux-x86_64-v1.1.0.tar.gz`)
3. Creates a GitHub Release for the tag and uploads all archives

The release is available at:
`https://github.com/rywils/fafind/releases/tag/v1.1.0`

Each Linux tarball contains one `fafind` binary. The AUR package installs `faf` as a symlink to that binary.

---

## 3. Publish to AUR (`fafind-bin`)

Full checklist: [`packaging/aur/README.md`](packaging/aur/README.md)

After the GitHub release assets are live:

```sh
cd packaging/aur
./update-checksums.sh   # requires makepkg / updpkgsums on Arch
makepkg -si             # optional local smoke test
```

Push `PKGBUILD` and `.SRCINFO` to `aur@aur.archlinux.org:fafind-bin.git`:

```sh
git clone ssh://aur@aur.archlinux.org/fafind-bin.git
cd fafind-bin
cp /path/to/fafind/packaging/aur/PKGBUILD .
cp /path/to/fafind/packaging/aur/.SRCINFO .
git add PKGBUILD .SRCINFO
git commit -m "upg: fafind-bin 1.1.0"
git push
```

Linux release URLs used by the PKGBUILD:

| Arch | URL path |
|------|----------|
| x86_64 | `.../fafind-linux-x86_64-v1.1.0.tar.gz` |
| aarch64 | `.../fafind-linux-arm64-v1.1.0.tar.gz` |

---

## 4. Update Homebrew formula sha256

```sh
curl -sL https://github.com/rywils/fafind/releases/download/v1.1.0/fafind-macos-arm64-v1.1.0.tar.gz | sha256sum
```

Replace the hash or placeholder values in `fafind.rb` and bump `version`. Submit a PR to tap or run `brew bump-formula-pr` if using homebrew-core.
