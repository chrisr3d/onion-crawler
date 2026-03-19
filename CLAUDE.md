# onion-crawler

Rust learning project that extracts `.onion` addresses from Common Crawl WARC archives.

## Build & Run

```sh
cargo build              # compile
cargo run                # download 1 archive (default)
cargo run -- --limit 3   # download up to 3 archives
cargo run -- --limit 2 custom.paths  # custom paths file
```

## Constraints

- **Teaching project**: code should be idiomatic Rust with clear comments explaining concepts
- **Edition 2024** (rustc 1.91+)
- Progress tracked in `PROGRESS.md`

## Current State

Step 2 complete: downloads WARC archives from Common Crawl over HTTP with streaming I/O,
skip-if-exists logic, `--limit N` flag, and progress display. Uses `ureq` (blocking HTTP).

## Roadmap

1. ~~Project scaffolding — read WARC paths file~~ (done)
2. ~~HTTP download of WARC archives~~ (done)
3. Gzip decompression + WARC record parsing
4. Regex extraction of `.onion` addresses
5. Deduplication and output formatting
6. Concurrent downloads with async/tokio
