# Release process

## 1. Tag and push

Update the version in `Cargo.toml` and `Cargo.lock` (`cargo build --release`), commit, then:

```sh
git tag v1.0.0
git push origin v1.0.0
```

That's the only manual step. Everything else is automated.

---

## 2. What the GitHub Action does

On every `v*` tag push, `.github/workflows/release.yml`:

1. Builds release binaries for all four targets in parallel:
   - `x86_64-unknown-linux-gnu` (native, Ubuntu runner)
   - `aarch64-unknown-linux-gnu` (via `cross`, Ubuntu runner)
   - `x86_64-apple-darwin` (native, macOS 13 runner)
   - `aarch64-apple-darwin` (native, macOS 14 runner)
2. Packages each binary as `fafind-<target>.tar.gz`
3. Creates a GitHub Release for the tag and uploads all four archives

The release is available at:
`https://github.com/rywils/fafind/releases/tag/v1.0.0`

---

## 3. Update AUR sha256sums

After the release is published, compute the checksums:

```sh
curl -sL https://github.com/rywils/fafind/releases/download/v1.0.0/fafind-x86_64-unknown-linux-gnu.tar.gz | sha256sum
curl -sL https://github.com/rywils/fafind/releases/download/v1.0.0/fafind-aarch64-unknown-linux-gnu.tar.gz | sha256sum
```

Paste the results into `PKGBUILD`:

```
sha256sums_x86_64=('<hash>')
sha256sums_aarch64=('<hash>')
```

Then regenerate `.SRCINFO`:

```sh
makepkg --printsrcinfo > .SRCINFO
```

Push both files to the AUR git repository.

---

## 4. Update Homebrew formula sha256

```sh
curl -sL https://github.com/rywils/fafind/releases/download/v1.0.0/fafind-x86_64-apple-darwin.tar.gz | sha256sum
curl -sL https://github.com/rywils/fafind/releases/download/v1.0.0/fafind-aarch64-apple-darwin.tar.gz | sha256sum
```

Replace the `PLACEHOLDER_*` values in `fafind.rb` and bump `version`. Submit a PR to tap or run `brew bump-formula-pr` if using homebrew-core.
