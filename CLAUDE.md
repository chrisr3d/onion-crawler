# onion-crawler

Rust learning project that extracts `.onion` addresses from Common Crawl WARC archives.

## Build & Run

```sh
cargo build                                    # compile
cargo run -- warc.paths                        # download & parse all archives
cargo run -- warc.paths.gz                     # same, from gzipped paths file
cargo run -- warc.paths -l 3                   # process up to 3 archives
cargo run -- warc.paths -j 2                   # 2 concurrent downloads (default: CPU cores)
cargo run -- warc.paths -d                     # delete archive after parsing
cargo run -- warc.paths -s memchr              # select parsing strategy (default)
cargo run -- warc.paths -s baseline            # original warc crate + regex
cargo run -- warc.paths -s bytes               # custom parser + regex::bytes
cargo run -- warc.paths -s mmap                # mmap + memchr (needs disk space)
cargo run -- warc.paths -m                     # parse from memory (no disk I/O)
cargo run -- warc.paths -s bytes --stream      # stream mode with bytes strategy
cargo run -- warc.paths.gz -l 3 -j 2 -d       # combined (short flags)
cargo run -- warc.paths.gz -l 3 -j 2 -m -s memchr  # combined with stream mode
cargo run -- warc.paths --limit 3 --jobs 2 --delete --strategy memchr  # combined (long flags)
cargo run -- --help                            # show usage and all options
```

## Constraints

- **Teaching project**: code should be idiomatic Rust with clear comments explaining concepts
- **Edition 2024** (rustc 1.91+)
- Progress tracked in `PROGRESS.md`

## Current State

Steps 1–9 complete: full async pipeline from reading WARC paths → concurrent HTTP
downloads → pipelined WARC parsing → `.onion` extraction → deduplication → JSON
output. Three-state processing model (processed → skip, downloaded → parse, missing →
download + parse). Multiple archives download in parallel (configurable `-j N`,
default: CPU core count), and parsing starts immediately when each download completes via
`spawn_blocking`. Layered resource guards cap concurrency at 2× CPU cores and available
RAM (stream mode). Results stored in `output/onions.json` as nested JSON with source
metadata (URL, date, archive), processing state tracked in `output/processed.log`.

Code is split into four files:
- `src/main.rs` — CLI parsing + pipeline orchestration + strategy dispatch
- `src/lib.rs` — download, parse functions (4 strategies + 2 in-memory variants), and state management
- `src/warc_parser.rs` — custom byte-level WARC parser (streaming + mmap slice)
- `src/onion_search.rs` — onion extraction strategies (regex-bytes, memchr)

CLI uses `clap` derive for argument parsing with short flags (`-l`, `-j`, `-d`, `-s`, `-m`)
and auto-generated `--help`. Input paths file is a required positional argument and
supports gzip-compressed `.gz` files (decompressed transparently via `libflate`).
Per-archive timing reports download and parse durations, with averages in the final summary.

### Parsing Strategies (Step 7)

Four ripgrep-inspired parsing strategies, selectable at runtime via `-s` / `--strategy`:

| Strategy | Parser | Search | Performance |
|----------|--------|--------|-------------|
| `baseline` | `warc` crate | `regex::Regex` on UTF-8 `String` | ~16s/GB (reference) |
| `bytes` | Custom streaming | `regex::bytes::Regex` on `&[u8]` | ~1.5-2x faster |
| `memchr` (default) | Custom streaming | SIMD `memmem` literal search | ~3-5x faster |
| `mmap` | Custom slice (mmap) | SIMD `memmem` + zero-copy bodies | ~3-6x faster |

Key optimizations over the baseline:
- **Custom WARC parser** skips non-response record bodies (~60% of data never read)
- **`regex::bytes`** eliminates `String::from_utf8_lossy` allocation per record
- **`memchr::memmem`** uses SIMD to scan for `.onion` literals at 16-32 bytes/cycle,
  replacing the regex NFA engine entirely
- **mmap variant** decompresses to temp file, memory-maps it, returns zero-copy `&[u8]`
  body slices (trades disk space for speed)
- **`flate2` with `zlib-ng`** backend for SIMD-optimized gzip decompression

### WARC Metadata Extraction (Step 8)

Each `.onion` address is stored with its source context as an `OnionSource` struct:
`url` (WARC-Target-URI — the crawled page), `date` (WARC-Date — crawl timestamp),
and `archive` (the WARC filename). Output is nested JSON: each onion maps to a list
of `OnionSource` objects, deduplicated on `(url, archive)` pairs.

The custom WARC parser (`warc_parser.rs`) extracts `WARC-Target-URI` and `WARC-Date`
headers alongside `WARC-Type` and `Content-Length`. The baseline strategy reads them
from the `warc` crate's parsed headers. `OnionSource` derives `Serialize`/`Deserialize`
via `serde` for JSON persistence. `load_results()` gracefully falls back to an empty
map if the file contains the old format.

### In-Memory Streaming Mode (Step 9)

The `--stream` / `-m` flag downloads archives into a `Vec<u8>` in memory instead of
writing to disk, then parses directly from that buffer using `std::io::Cursor`. This
eliminates all disk I/O in the download/parse pipeline.

Compatible with `bytes` and `memchr` strategies (which use the generic
`WarcRecordIter<R: BufRead>` parser). Incompatible strategies (`baseline`, `mmap`)
print a warning and fall back to disk mode. Memory cost: ~800 MB per archive ×
concurrent jobs.

Key concepts: `Cursor<Vec<u8>>` as a `Read` adapter (same interface as `File`),
`Vec::with_capacity` pre-allocation from `Content-Length`, zero-cost ownership
transfer of the buffer via `move` into `spawn_blocking`.

### Resource Guards (Step 9b)

Layered concurrency caps prevent oversubscription and OOM:

1. **CPU cap**: `min(config.jobs, 2 × cpu_cores)` — beyond 2× cores, `spawn_blocking`
   tasks queue up adding context switching overhead without throughput gain.
2. **RAM cap** (stream mode only): `min(capped_jobs, available_ram / 1 GB)` — each
   in-memory archive costs ~800 MB–1 GB; exceeding available RAM causes OOM.
3. **`buffer_unordered(effective_jobs)`** — natural backpressure: finished jobs free
   their buffers before new ones start.

`effective_jobs = min(config.jobs, 2 × cores, available_ram / 1 GB)` — the most
restrictive limit wins. In disk mode, the RAM cap is skipped.

`get_available_memory()` uses `#[cfg(target_os)]` conditional compilation with three
platform-specific implementations (all safe Rust, no `unsafe`):
- **macOS**: `std::process::Command` running `sysctl -n` for page size + free/speculative/purgeable pages
- **Linux**: parsing `MemAvailable` from `/proc/meminfo`
- **Other**: returns `None` (skips RAM check, proceeds with CPU-capped jobs)

Dependencies: `clap` (CLI parsing), `reqwest` (async HTTP), `tokio` (async runtime),
`futures` (stream utilities), `warc` (baseline WARC parsing), `regex`, `serde`,
`serde_json`, `libflate` (gzip for paths file), `flate2` with `zlib-ng` (gzip for
custom parser), `memchr` (SIMD literal search), `memmap2` (memory-mapped I/O).

Performance: `[profile.dev.package."*"] opt-level = 2` optimizes dependencies in debug
builds. Regex is case-sensitive (`(?i)` flag causes 1000x+ slowdown with `\b` boundaries).
With `memchr` strategy + `zlib-ng`, release build parses an 864MB archive in ~5.6s
(vs ~4.3s for `rg -z` on the same file).

## Roadmap

1. ~~Project scaffolding — read WARC paths file~~ (done)
2. ~~HTTP download of WARC archives~~ (done)
3. ~~Gzip decompression + WARC record parsing~~ (done)
4. ~~Regex extraction of `.onion` addresses~~ (done)
5. ~~Deduplication and output formatting~~ (done)
6. ~~Concurrent downloads with async/tokio~~ (done)
7. ~~Ripgrep-style parsing strategies~~ (done)
8. ~~WARC metadata extraction~~ (done)
9. ~~In-memory streaming mode~~ (done)
