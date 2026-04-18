# fafind

### fast as f*#! filename search.

`fafind` is a zero-allocation, parallel filesystem search tool written in Rust, focused purely on filename matching

It’s built to rip through millions of files with minimal overhead.

---

## why this exists

Most search tools either:

- scan file contents (slow for this use case),  
- allocate constantly, or  
- bottleneck on output or synchronization  

**fafind avoids all of that.**

This is a hot-path optimized walker with:

- zero allocations per entry  
- lock-free worker output  
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
sudo cp target/release/fafind /usr/local/bin/
~~~

---

## usage

~~~bash
fafind <target> [root]
~~~

If `root` is not provided, it defaults to `/`.

---

## matching modes

### default (stem match)

Matches filename **without extension**.

Example:
~~~bash
fafind main .
~~~

Matches:
- main.rs  
- main.go  

Does NOT match:
- domain.rs  

---

### substring match (`-s`)

Example:
~~~bash
fafind -s foo .
~~~

Matches:
- foobar.txt  
- myfoo.rs  
- prefoo  

---

### exact match (`-p`)

Example:
~~~bash
fafind -p Makefile .
~~~

Matches:
- Makefile  

Only exact filename match (including extension).

---

## flags

### case insensitive (`-i` / `--ignore-case`)

~~~bash
fafind -i readme .
~~~

---

### limit depth

~~~bash
fafind --max-depth 3 main .
~~~

---

### exclude directories

~~~bash
fafind --exclude target,node_modules main .
~~~

---

### respect .gitignore

~~~bash
fafind --gitignore main .
~~~

---

### filter by type

~~~bash
fafind --type f main .   # files only
fafind --type d src .    # directories only
fafind --type a main .   # any (default)
~~~

---

### null-separated output (`-0` / `--null`)

~~~bash
fafind -0 main . | xargs -0 rm
~~~

---

### verbose mode

~~~bash
fafind -v main .
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
fafind -q main .
~~~

---

## example

~~~bash
fafind -i --exclude target,node_modules --max-depth 5 main .
~~~

---

## performance characteristics

### zero allocation hot path

- no heap usage per file  
- stack buffers for ASCII matching  
- fallback only when necessary  

### parallel by default

- uses all available CPU cores  
- work-stealing via `ignore::WalkBuilder`  

### lock-free output

- each worker writes to its own buffer  
- merged once at the end  
- no contention during traversal  

### ASCII fast path

- ~95% of filenames handled without Unicode overhead  

### SIMD substring search

- powered by `memchr::memmem`  

---

## implementation details

- `clap` for CLI parsing  
- `ignore` for parallel walking  
- `memchr` for fast substring search  
- `smallvec` for stack-allocated exclude lists  

### key design decisions

- no channels  
- no shared queues  
- no per-match locks  
- no unnecessary syscalls  
- no UTF-8 conversion on Unix (raw bytes)  

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

## license

MIT