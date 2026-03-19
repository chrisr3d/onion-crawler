// onion-crawler — Step 2: Download WARC archives from Common Crawl
//
// Reads a list of WARC archive paths, then downloads each one over HTTP.
// Archives are streamed to disk (never loaded entirely into memory) with
// progress output. Already-downloaded files are skipped automatically.
//
// New concepts in this step: external crates, Box<dyn Error>, the ? operator,
// Path/PathBuf, fs::create_dir_all, impl Trait parameters, Read/Write traits,
// iterator .take(), and str::parse::<T>().

use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

/// Base URL for Common Crawl data archives.
const COMMONCRAWL_BASE: &str = "https://data.commoncrawl.org/";

/// Default number of archives to download per run.
const DEFAULT_LIMIT: usize = 1;

// ---------------------------------------------------------------------------
// CLI Argument Parsing
// ---------------------------------------------------------------------------

/// Parse command-line arguments into (paths_file, limit).
///
/// Supports:
///   --limit N       or  --limit=N   → cap how many files to download
///   <positional>                     → path to the WARC-paths file
///
/// Returns a tuple — Rust functions can return multiple values this way.
/// `String` is an owned, heap-allocated string; `usize` is an unsigned
/// pointer-sized integer (ideal for counts and indices).
fn parse_args() -> (String, usize) {
    // `env::args()` returns an iterator of Strings. `.skip(1)` drops the
    // program name (argv[0]). `.collect()` consumes the iterator into a Vec.
    let args: Vec<String> = env::args().skip(1).collect();

    let mut paths_file = String::from("warc.paths");
    let mut limit = DEFAULT_LIMIT;

    // We walk through args with an index so we can peek ahead for `--limit N`.
    let mut i = 0;
    while i < args.len() {
        // `str::strip_prefix` returns Option<&str> — the remainder after the
        // prefix, or None if the string doesn't start with it.
        if let Some(value) = args[i].strip_prefix("--limit=") {
            // `str::parse::<usize>()` tries to convert a string to a number.
            // It returns Result<usize, ParseIntError>. We handle the error
            // with `unwrap_or_else` to give a clear message.
            limit = value.parse::<usize>().unwrap_or_else(|_| {
                eprintln!("Error: --limit value '{}' is not a valid number", value);
                process::exit(1);
            });
        } else if args[i] == "--limit" {
            // `--limit N` form: the value is in the next argument.
            i += 1;
            if i >= args.len() {
                eprintln!("Error: --limit requires a value");
                process::exit(1);
            }
            limit = args[i].parse::<usize>().unwrap_or_else(|_| {
                eprintln!("Error: --limit value '{}' is not a valid number", args[i]);
                process::exit(1);
            });
        } else if args[i].starts_with('-') {
            eprintln!("Error: unknown flag '{}'", args[i]);
            process::exit(1);
        } else {
            // Positional argument — treat as the paths file.
            // `.clone()` creates an owned copy of the String.
            paths_file = args[i].clone();
        }
        i += 1;
    }

    (paths_file, limit)
}

// ---------------------------------------------------------------------------
// Download Logic
// ---------------------------------------------------------------------------

/// Download a single WARC archive from Common Crawl.
///
/// `uri` is the relative path from `warc.paths` (e.g.
/// `crawl-data/CC-NEWS/2025/10/CC-NEWS-20251001002341-04358.warc.gz`).
///
/// ## Return type: `Result<(), Box<dyn Error>>`
///
/// `Box<dyn Error>` is a *trait object* — a heap-allocated value that
/// implements the `Error` trait. This lets the function return *any* error
/// type (io::Error, ureq::Error, etc.) without the caller needing to know
/// which concrete type it is. The `?` operator converts compatible errors
/// automatically.
/// Returns `Ok(true)` if a file was actually downloaded, `Ok(false)` if
/// it was skipped because it already exists on disk.
fn download_warc(uri: &str) -> Result<bool, Box<dyn Error>> {
    // --- Build the full URL ---
    let url = format!("{}{}", COMMONCRAWL_BASE, uri);

    // --- Extract the filename ---
    // `rsplit('/')` splits from the right; `.next()` gives the last segment.
    // `ok_or("...")` converts Option → Result, so we can use `?`.
    let filename = uri
        .rsplit('/')
        .next()
        .ok_or("URI has no filename component")?;

    // --- Build local path ---
    // `Path` is a borrowed path slice (like &str for file paths).
    // `PathBuf` is the owned version (like String). `.join()` appends
    // a component with the correct OS separator.
    let local_path: PathBuf = Path::new("download").join(filename);

    // --- Skip if already downloaded ---
    if local_path.exists() {
        eprintln!("  Skipping {} (already exists)", filename);
        return Ok(false);
    }

    // --- Create download directory ---
    // `create_dir_all` is like `mkdir -p`: creates the directory and all
    // parent directories if they don't exist. Succeeds silently if the
    // directory is already there.
    fs::create_dir_all("download")?;

    eprintln!("  Downloading {}", filename);
    eprintln!("  URL: {}", url);

    // --- Build an HTTP agent with appropriate timeouts ---
    // `ureq::Agent` is a connection pool + configuration bundle.
    // We disable the global timeout (which would cap the entire transfer)
    // but keep a connect timeout so we fail fast on unreachable hosts.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(None)
        .timeout_connect(Some(Duration::from_secs(30)))
        .build()
        .new_agent();

    // --- Make the HTTP GET request ---
    // `.call()` sends the request and returns the response. The `?` operator
    // propagates any error (DNS failure, timeout, HTTP 4xx/5xx) to our caller.
    let response = agent.get(&url).call()?;

    // --- Read Content-Length for progress display ---
    // The server may or may not include this header. We use Option<u64>.
    let total_size = response.body().content_length();

    if let Some(size) = total_size {
        eprintln!("  Size: {:.1} MB", size as f64 / 1_048_576.0);
    }

    // --- Stream the body to a file ---
    // `into_body()` consumes the response and gives us the Body.
    // `.with_config().limit(u64::MAX)` removes the default 10 MB read limit
    // (ureq v3 enforces this by default for safety).
    // `.reader()` gives us a type that implements `std::io::Read`.
    //
    // We split the method chain into separate bindings because `.reader()`
    // borrows from the Body — if Body were a temporary, the borrow would
    // dangle. This is Rust's borrow checker keeping us safe: each intermediate
    // value must live long enough for the next step to borrow from it.
    let mut body = response.into_body();
    let configured = body.with_config().limit(u64::MAX);
    let body_reader = configured.reader();

    // `File::create` opens a file for writing, creating it if it doesn't
    // exist or truncating it if it does. Contrast with `File::open` which
    // opens for reading only.
    let writer = File::create(&local_path)?;

    let bytes_written = download_with_progress(body_reader, writer, total_size)?;

    eprintln!(
        "  Done: {:.1} MB written to {}",
        bytes_written as f64 / 1_048_576.0,
        local_path.display()
    );

    Ok(true)
}

/// Stream bytes from `reader` to `writer` with periodic progress output.
///
/// ## `impl Read` / `impl Write` — trait bounds as parameters
///
/// Instead of accepting a concrete type like `File`, this function accepts
/// *anything that implements `Read`* and *anything that implements `Write`*.
/// This makes the function reusable (e.g., we could write to a Vec<u8> in
/// tests) and is Rust's way of achieving polymorphism at compile time.
///
/// Returns the total number of bytes copied.
fn download_with_progress(
    mut reader: impl Read,
    mut writer: impl Write,
    total_size: Option<u64>,
) -> Result<u64, io::Error> {
    // Stack-allocated buffer — 64 KB is a good balance between syscall
    // overhead and memory usage. This lives on the stack, not the heap,
    // so there's no allocation cost.
    let mut buf = [0u8; 65_536];

    let mut bytes_written: u64 = 0;
    let mut last_report: u64 = 0;

    loop {
        // `.read()` fills the buffer with up to buf.len() bytes and
        // returns how many were actually read. Returns 0 at EOF.
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }

        // `.write_all()` writes the entire slice, retrying as needed.
        // Contrast with `.write()` which may write fewer bytes than requested.
        writer.write_all(&buf[..n])?;
        bytes_written += n as u64;

        // Print progress every ~10 MB to avoid flooding the terminal.
        if bytes_written - last_report >= 10 * 1_048_576 {
            last_report = bytes_written;
            match total_size {
                Some(total) => {
                    let pct = (bytes_written as f64 / total as f64) * 100.0;
                    eprint!(
                        "\r  Progress: {:.1} MB / {:.1} MB ({:.0}%)",
                        bytes_written as f64 / 1_048_576.0,
                        total as f64 / 1_048_576.0,
                        pct,
                    );
                }
                None => {
                    eprint!(
                        "\r  Progress: {:.1} MB",
                        bytes_written as f64 / 1_048_576.0,
                    );
                }
            }
        }
    }

    // Clear the progress line if we printed any progress
    if last_report > 0 {
        eprintln!();
    }

    Ok(bytes_written)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // --- Parse CLI arguments ---
    let (paths_file, limit) = parse_args();

    eprintln!(
        "Reading WARC paths from '{}' (limit: {})",
        paths_file, limit
    );

    // --- Open and read the paths file (Step 1 logic, refined) ---
    let file = File::open(&paths_file).unwrap_or_else(|err| {
        eprintln!("Error: cannot open '{}': {}", paths_file, err);
        process::exit(1);
    });
    let reader = BufReader::new(file);

    // --- Build the list of URIs ---
    // `.lines()` yields Result<String> lazily.
    // `.filter_map(Result::ok)` silently drops lines that fail to read.
    // `.map(|l| ...)` trims whitespace from each line.
    // `.filter(|l| ...)` drops empty lines.
    let uris: Vec<String> = reader
        .lines()
        .filter_map(Result::ok)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if uris.is_empty() {
        eprintln!("No paths found in '{}'", paths_file);
        process::exit(1);
    }

    eprintln!("Found {} path(s) in file\n", uris.len());

    // --- Download loop ---
    // We iterate through *all* URIs but only count actual downloads
    // towards the limit. Already-downloaded files are skipped for free,
    // so the program resumes where it left off on re-runs.
    let mut download_count = 0;
    let mut skip_count = 0;
    let mut fail_count = 0;

    for (i, uri) in uris.iter().enumerate() {
        // Stop once we've downloaded enough *new* files.
        if download_count >= limit {
            eprintln!(
                "Reached download limit of {}. Stopping.",
                limit
            );
            break;
        }

        eprintln!("[{}] {}", i + 1, uri);

        match download_warc(uri) {
            Ok(true) => {
                // Actually downloaded a new file — counts towards limit.
                download_count += 1;
            }
            Ok(false) => {
                // File already existed — skip doesn't count towards limit.
                skip_count += 1;
            }
            Err(err) => {
                eprintln!("  Error: {}", err);
                fail_count += 1;
            }
        }
        eprintln!();
    }

    // --- Summary ---
    eprintln!(
        "Summary: {} downloaded, {} skipped, {} failed",
        download_count, skip_count, fail_count
    );
}
