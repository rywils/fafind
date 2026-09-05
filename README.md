# fafind / faf

### fast as f*#! filename search.

`fafind` (alias `faf`) is a parallel filesystem search tool written in Rust.
It matches filenames only and stays out of file contents.

---

Pair it with [delfaf](https://github.com/rywils/delfaf) to mass-delete whatever the last `faf` run found.

---

## why this exists

Most search tools scan file contents, allocate per entry, or stall on output locks.
faf walks the tree on every core, matches raw filename bytes, and writes results in large batches.

---

## install

### from source

~~~bash
git clone https://github.com/rywils/fafind
cd fafind
cargo build --release
sudo cp target/release/faf target/release/fafind /usr/local/bin/
~~~

`faf` and `fafind` are the same program.
Use whichever name you prefer.

### packages

- Arch (AUR): `fafind-bin`, see [`packaging/aur/README.md`](packaging/aur/README.md)
- Homebrew: see `RELEASE.md`

The AUR package installs `fafind` and a `faf` symlink.

---

## usage

~~~bash
faf <target> [root]
~~~

`root` defaults to `/`.

---

## matching modes

### default (stem match)

Matches the filename without its extension.
An extension on the query is ignored, so `faf main.rs` is the same as `faf main`.

~~~bash
faf main .
~~~

Matches `main.rs` and `main.go`.
Does not match `domain.rs`.

### substring (`-s`)

~~~bash
faf -s foo .
~~~

Matches `foobar.txt`, `myfoo.rs`, `prefoo`, and `notes.foo`.
The whole filename is searched, extension included.

### exact (`-p`)

~~~bash
faf -p Makefile .
~~~

Matches `Makefile` only.

---

## terminal colors

When stdout is a terminal and `-0` is not set, matches are highlighted:

| Color | Applies to |
|-------|------------|
| Dim | Path before the filename |
| Green | The matched part of the name |
| Bold green | Stem in `-p` mode |
| Yellow | Extension in stem and `-p` modes |
| Orange | Non-matching parts of the name in `-s` mode |

`--color auto` is the default.
Use `--color always` or `--color never` to override.

---

## flags

### case insensitive (`-i`)

~~~bash
faf -i readme .
~~~

ASCII names use a byte-wise fold.
Non-ASCII names and queries fall back to full Unicode case folding.

### limit depth

~~~bash
faf --max-depth 3 main .
~~~

### exclude directories

~~~bash
faf --exclude target,node_modules main .
~~~

Matches directory names anywhere below the root.
The root itself is never excluded.

### respect .gitignore

~~~bash
faf --gitignore main .
~~~

### filter by type (`-f` / `-d` / `--type`)

~~~bash
faf -f main .         # files only
faf -d src .          # directories only
faf --type a main .   # any (default)
~~~

`-f` and `-d` stack with the other short flags, so `-sd`, `-pf`, and `-id` all work.
They cannot be combined with each other or contradict an explicit `--type`.

### null-separated output (`-0`)

~~~bash
faf -0 main . | xargs -0 rm
~~~

Disables color.

### verbose (`-v`)

Prints `[SCAN]`, `[SKIP]`, and `[ERROR]` lines to stderr.
Matches still go to stdout.

### quiet (`-q`)

Suppresses the summary line on stderr.

---

## performance

- Every core walks the tree through a work-stealing scheduler.
- Filenames are matched as raw bytes, with no UTF-8 decoding or per-entry allocation in the matcher.
- Substring search uses a prebuilt SIMD `memmem` finder.
- Non-ASCII names take a cold Unicode path only when `-i` needs it.
- Each worker batches output into a private buffer and writes it in 64 KiB chunks, whether stdout is a terminal or a pipe.
- When the reader closes the pipe, as in `faf -s foo / | head`, the walk stops instead of scanning the rest of the disk.

---

## output

- newline-separated by default, NUL-separated with `-0`
- raw OS bytes, no re-encoding
- the matched paths are also written to `~/.cache/fafind/last` for `delfaf`

---

## exit codes

~~~text
0 = matches found
1 = no matches
2 = invalid usage
~~~

---

## what this is NOT

- not a content search tool (use `grep` or `rg`)
- not a fuzzy matcher

---

## changelog

See [CHANGELOG.md](CHANGELOG.md).

## license

MIT
