# onion-crawler

A Rust program that extracts `.onion` addresses from [Common Crawl](https://commoncrawl.org/) WARC archives.

This project is also a **hands-on Rust tutorial** — each step introduces new language concepts with well-commented code and a learning journal (`PROGRESS.md`) explaining the reasoning behind every pattern used.

## Purpose

Common Crawl provides petabytes of web archive data stored as WARC (Web ARChive) files. This tool downloads those archives, parses them, and extracts any `.onion` (Tor hidden service) addresses found in the crawled content.

## Features

The project was built incrementally and now features the following:

1. **Read WARC paths** — Parse an input file listing WARC archive URIs (plain text or `.gz`)
2. **HTTP download** — Fetch WARC archives from Common Crawl servers
3. **WARC parsing** — Decompress gzip-compressed archives and parse WARC records
4. **Onion extraction** — Use regex to find `.onion` addresses in page content
5. **Deduplication & output** — Deduplicate results and produce structured JSON output
6. **Concurrent processing** — Download and parse multiple archives in parallel with async/tokio
7. **Ripgrep-style parsing strategies** — Four optimization layers selectable at runtime (`-s`)
8. **WARC metadata extraction** — Track source URL, crawl date, and archive for each `.onion` found

Additional features:

8. **CLI with `clap`** — Short flags (`-l`, `-j`, `-d`, `-s`), auto `--help`, required input file
9. **Timing** — Per-archive download/parse duration and averages in summary

## Rust Concepts Covered

Each step introduces new Rust fundamentals:

- Error handling with `Result` and `Option`
- Buffered I/O and iterators
- Pattern matching
- Adding and using external crates
- Async programming with `tokio`
- Regex and string processing
- Concurrency patterns
- Serialization with `serde` derive macros
- SIMD-accelerated byte searching (`memchr`)
- Memory-mapped I/O (`mmap`)
- `unsafe` blocks and when they're justified
- Performance optimization and benchmarking strategies

See `[PROGRESS.md](documentation/PROGRESS.md)` for detailed explanations of each concept as they are introduced.

## Build & Run

Requires Rust 1.91+ (edition 2024).

```sh
cargo build                                    # compile (debug)
cargo build --release                          # compile (optimized — ~5.6s per 864MB archive)
cargo run -- warc.paths                        # download & parse all archives
cargo run -- warc.paths.gz                     # same, from gzipped paths file
cargo run -- warc.paths -l 3                   # process up to 3 archives
cargo run -- warc.paths -j 2                   # 2 concurrent downloads (default: CPU cores)
cargo run -- warc.paths -d                     # delete archive after parsing
cargo run -- warc.paths -s memchr              # select parsing strategy (default)
cargo run -- warc.paths -s baseline            # original warc crate + regex
cargo run -- warc.paths -s bytes               # custom parser + regex::bytes
cargo run -- warc.paths -s mmap                # mmap + memchr (needs disk space)
cargo run -- warc.paths.gz -l 3 -j 2 -d       # combined (short flags)
cargo run -- warc.paths --limit 3 --jobs 2 --delete --strategy memchr  # combined (long flags)
cargo run -- --help                            # show usage and all options
```

## Parsing Strategies

Four ripgrep-inspired optimization layers, selectable at runtime via `-s` / `--strategy`:

| Strategy | Technique | Speedup vs baseline |
|----------|-----------|---------------------|
| `baseline` | `warc` crate + `regex::Regex` on UTF-8 strings | 1x (~16s/GB) |
| `bytes` | Custom WARC parser + `regex::bytes` on raw `&[u8]` | ~1.5-2x |
| `memchr` (default) | Custom parser + SIMD `memmem` literal search | ~3-5x |
| `mmap` | Decompress to temp file + mmap + zero-copy memchr | ~3-6x |

Key optimizations: custom WARC parser skips non-response bodies (~60% of data), SIMD
literal search replaces the regex engine, `zlib-ng` backend for faster gzip decompression.

## Output Format

Results are stored in `output/onions.json` as nested JSON. Each `.onion` address maps to
a list of sources with the clearnet URL where it was found, the crawl date, and the archive:

```json
{
  "example1234567890.onion": [
    {
      "url": "https://example.com/page",
      "date": "2024-09-15T12:34:56Z",
      "archive": "CC-NEWS-20240915.warc.gz"
    }
  ]
}
```

Source metadata is extracted from WARC headers (`WARC-Target-URI`, `WARC-Date`) during
parsing. Results are deduplicated on `(url, archive)` pairs.
