# AUR: fafind-bin

Pre-built binaries from [GitHub Releases](https://github.com/rywils/fafind/releases). Installs:

- `/usr/bin/fafind` — release binary
- `/usr/bin/faf` — symlink to `fafind` (same program; `argv[0]` selects the summary label)

## Maintainer checklist (v1.1.0+)

### 1. Publish upstream release

From the repo root:

```sh
git tag v1.2.0
git push origin v1.2.0
```

Wait for [release.yml](../../.github/workflows/release.yml) to finish. Confirm these assets exist:

| Architecture | Asset filename |
|--------------|----------------|
| x86_64 Linux | `fafind-linux-x86_64-v1.2.0.tar.gz` |
| aarch64 Linux | `fafind-linux-arm64-v1.2.0.tar.gz` |

Each tarball contains a single `fafind` executable (stripped).

### 2. Refresh checksums

On Arch Linux (or an Arch container) in this directory:

```sh
cd packaging/aur
./update-checksums.sh
```

This runs `updpkgsums` and regenerates `.SRCINFO`. Commit both files.

If `updpkgsums` fails because the tag is not up yet, fix `pkgver` / URLs in `PKGBUILD` first, then retry.

### 3. Test the package

```sh
makepkg -si
faf --version
fafind --version
faf main /usr/share/doc  # quick smoke test
```

Or in a clean chroot:

```sh
extra-x86_64-build
```

### 4. Push to AUR

```sh
git clone ssh://aur@aur.archlinux.org/fafind-bin.git
cd fafind-bin
cp /path/to/fafind/packaging/aur/PKGBUILD .
cp /path/to/fafind/packaging/aur/.SRCINFO .
git add PKGBUILD .SRCINFO
git commit -m "upg: fafind-bin 1.2.0"
git push
```

For a **new** AUR package, create the empty repo on AUR first (web UI), then push the initial `PKGBUILD` and `.SRCINFO`.

### 5. Version bumps later

1. Bump `pkgver` / `pkgrel` in `PKGBUILD`
2. Run `./update-checksums.sh` after the matching GitHub release exists
3. Test with `makepkg -si`
4. Push to `aur@aur.archlinux.org:fafind-bin.git`

## Files

| File | Purpose |
|------|---------|
| `PKGBUILD` | Package recipe |
| `.SRCINFO` | AUR metadata (must match `PKGBUILD`; regenerate, do not hand-edit for releases) |
| `update-checksums.sh` | `updpkgsums` + `makepkg --printsrcinfo` |
| `LICENSE` | REUSE metadata reference (upstream license is fetched in `PKGBUILD`) |
