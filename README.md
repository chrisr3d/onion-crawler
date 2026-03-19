# onion-crawler

A Rust program that extracts `.onion` addresses from [Common Crawl](https://commoncrawl.org/) WARC archives.

This project is also a **hands-on Rust tutorial** — each step introduces new language concepts with well-commented code and a learning journal (`PROGRESS.md`) explaining the reasoning behind every pattern used.

## Purpose

Common Crawl provides petabytes of web archive data stored as WARC (Web ARChive) files. This tool downloads those archives, parses them, and extracts any `.onion` (Tor hidden service) addresses found in the crawled content.

## Planned Features

The project is built incrementally, one feature per step:

1. **Read WARC paths** — Parse an input file listing WARC archive URIs
2. **HTTP download** — Fetch a single WARC archive from Common Crawl servers
3. **WARC parsing** — Decompress gzip-compressed archives and parse WARC records
4. **Onion extraction** — Use regex to find `.onion` addresses in page content
5. **Deduplication & output** — Deduplicate results and produce clean output
6. **Concurrent downloads** — Process multiple archives in parallel with async/tokio

## Rust Concepts Covered

Each step introduces new Rust fundamentals:

- Error handling with `Result` and `Option`
- Buffered I/O and iterators
- Pattern matching
- Adding and using external crates
- Async programming with `tokio`
- Regex and string processing
- Concurrency patterns

See `[PROGRESS.md](documentation/PROGRESS.md)` for detailed explanations of each concept as they are introduced.

## Build & Run

Requires Rust 1.91+ (edition 2024).

```sh
cargo build              # compile
cargo run                # run with default warc.paths
cargo run -- <file>      # run with a custom WARC paths file
```
