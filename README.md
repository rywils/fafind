# fafind / faf

### fast as f*#! filename search.

`fafind` (and the shorter alias **`faf`**) is a zero-allocation, parallel filesystem search tool written in Rust, focused purely on filename matching.

It’s built to rip through millions of files with minimal overhead.

---

## why this exists

Most search tools either:

- scan file contents (slow for this use case),
- allocate constantly, or
- bottleneck on output or synchronization

**faf avoids all of that.**

This is a hot-path optimized walker with:

- zero allocations per entry
- SIMD substring matching
- ASCII fast paths with Unicode fallback
- parallel traversal using a work-stealing scheduler

---

## install

### from source

~~~bash
git clone https://github.com/rywils/fafind
cd fafind
cargo build --release
sudo cp target/release/faf target/release/fafind /usr/local/bin/
~~~

Both `faf` and `fafind` are built from the same codebase. Use whichever name you prefer.

### packages

- **Arch (AUR):** `fafind-bin` — see [`packaging/aur/README.md`](packaging/aur/README.md)
- **Homebrew:** see `RELEASE.md`

The AUR package installs `fafind` and a `faf` symlink. From source, copy or link both names as you prefer.

---

## usage

~~~bash
faf <target> [root]
# same as:
fafind <target> [root]
~~~

If `root` is not provided, it defaults to `/`.

---

## matching modes

### default (stem match)

Matches filename **without extension**.

~~~bash
faf main .
~~~

Matches:

- `main.rs`
- `main.go`

Does NOT match:

- `domain.rs`

---

### substring match (`-s`)

~~~bash
faf -s foo .
~~~

Matches:

- `foobar.txt`
- `myfoo.rs`
- `prefoo`

---

### exact match (`-p`)

~~~bash
faf -p Makefile .
~~~

Matches:

- `Makefile`

Only exact filename match (including extension).

---

## terminal colors

When stdout is a **terminal** (and not `-0` / `--null`), matches are highlighted:

| Color | Applies to |
|-------|------------|
| **Dim** | Path before the filename (`/path/to/`) |
| **Green** | The matched part of the name (stem in default/`-p`; each hit in `-s`) |
| **Bold green** | Stem in `-p` (exact) mode |
| **Yellow** | Extension (`.rs`, `.js`, `.docx`, …) |
| **Orange** | Non-matching parts of the name in `-s` only |

**Stem match** — `faf main .` on `/app/main.rs`: dim `/app/`, green `main`, yellow `.rs`

**Substring** — `faf -s main .` on `has_dot_entry_main_corner.js`:

- Orange: `has_dot_entry_` and `_corner`
- Green: `main`
- Yellow: `.js`

Control coloring with `--color auto` (default), `--color always`, or `--color never`. Colors are off when output is piped to a file or tool unless you force `--color always`.

---

## flags

### case insensitive (`-i` / `--ignore-case`)

~~~bash
faf -i readme .
~~~

---

### limit depth

~~~bash
faf --max-depth 3 main .
~~~

---

### exclude directories

~~~bash
faf --exclude target,node_modules main .
~~~

---

### respect .gitignore

~~~bash
faf --gitignore main .
~~~

---

### filter by type

~~~bash
faf --type f main .   # files only
faf --type d src .    # directories only
faf --type a main .   # any (default)
~~~

---

### null-separated output (`-0` / `--null`)

~~~bash
faf -0 main . | xargs -0 rm
~~~

Disables color highlighting.

---

### verbose mode

~~~bash
faf -v main .
~~~

Sends to **stderr**:

- `[SCAN]` for every visited entry
- `[SKIP]` for excluded directories
- `[ERROR]` for unreadable entries

Matches are still written to **stdout** as normal.

---

### quiet mode (`-q` / `--quiet`)

Suppresses the summary line printed to stderr after the search completes.

~~~bash
faf -q main .
~~~

---

## example

~~~bash
faf -i --exclude target,node_modules --max-depth 5 main .
~~~

---

## performance characteristics

### zero allocation hot path

- no heap usage per file
- stack buffers for ASCII matching
- fallback only when necessary
- raw reverse-byte scan for filename extraction on non-root entries,
  bypassing `Path::file_name()`'s `Components` parsing (the walk root still
  uses the general path, since it may be `.`, `/`, or user-supplied)
- filename stem computed once per entry in default (stem) match mode,
  shared between the length prefilter and the equality check

### parallel by default

- uses all available CPU cores
- work-stealing via `ignore::WalkBuilder`

### efficient output

- each worker formats matches into a private buffer
- **TTY:** batched stdout writes (64 KiB) to avoid locking on every match
- **pipes:** flush after each match line so scripts see results immediately
- atomic counters for scan/match totals (no mutex on the hot path)

### ASCII fast path

- ~95% of filenames handled without Unicode overhead

### SIMD substring search

- powered by `memchr::memmem`
- length prefilter skips obvious non-matches before full comparison

---

## implementation details

- `clap` for CLI parsing
- `ignore` for parallel walking
- `memchr` for fast substring search
- `smallvec` for stack-allocated exclude lists

### key design decisions

- no channels
- no shared match queues
- no UTF-8 conversion on Unix (raw bytes)
- matcher length prefilter and per-worker config caching

---

## output format

- newline-separated by default
- NUL-separated with `-0`
- raw OS bytes on Unix (no encoding overhead)

---

## exit codes

~~~text
0 = matches found
1 = no matches
2 = error / invalid usage
~~~

---

## what this is NOT

- not a content search tool (use `grep` or `rg`)
- not a fuzzy matcher
- not a UI tool

This is a fast, deterministic filename matcher.

---

## changelog

See [CHANGELOG.md](CHANGELOG.md).

---

## license

MIT
