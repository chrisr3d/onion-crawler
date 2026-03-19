# Progress Journal

## Step 1: Project Scaffolding — Read WARC Paths File

### What we built

A minimal Rust binary that reads a file of WARC archive paths (one per line) and prints
each path with its line number. Accepts any path file as a CLI argument, defaulting to
`warc.paths`. This is the foundation that all subsequent steps build on.

### Rust concepts introduced

**`std::env::args()` — CLI argument parsing**
Returns an iterator over the command-line arguments passed to the program. Index 0 is
the program name, index 1 is the first user argument. We use `.nth(1)` to grab it as
an `Option<String>`, then `unwrap_or_else` to provide a default value.

**`Result<T, E>` — Error handling without exceptions**
Rust has no exceptions. Functions that can fail return `Result`, which is either `Ok(value)`
or `Err(error)`. `File::open` returns `Result<File, io::Error>`. We handle the error case
explicitly with `unwrap_or_else`, printing to stderr and exiting with a non-zero code.

**`BufReader` — Buffered I/O**
Reading a file byte-by-byte (or line-by-line without buffering) triggers a system call per
read. `BufReader` wraps a `File` with an 8 KB in-memory buffer, so the OS reads in large
chunks and our code consumes from the buffer. Same result, far fewer syscalls.

**`.lines().enumerate()` — Iterator composition**
`.lines()` on a `BufReader` returns an iterator that lazily yields one `Result<String>` per
line. `.enumerate()` wraps each item as `(index, value)`. No vector is allocated — lines
are processed one at a time (this is what Rust calls a "zero-cost abstraction").

**`match` — Pattern matching**
Rust's `match` is an exhaustive pattern-matching expression. The compiler ensures every
variant is handled. We match on `Ok(line)` and `Err(err)` to process good lines and
report bad ones without crashing.

**`println!` vs `eprintln!` — stdout vs stderr**
`println!` writes to stdout (the data stream). `eprintln!` writes to stderr (diagnostics).
This separation matters when piping: `cargo run | head -5` shows only paths, not the
summary line.

**`{:>4}` — Format specifiers**
Rust's formatting mini-language in `println!` supports alignment and width. `{:>4}` means
"right-align in a 4-character field", producing neatly columned output like `   1 | ...`.

### What's next

Step 2: Download WARC archives over HTTP.

---

## Step 2: HTTP Download of WARC Archives

### What we built

An HTTP downloader that fetches Common Crawl WARC archives to a local `download/` directory.
Each URI from `warc.paths` is prepended with `https://data.commoncrawl.org/` to form the
full URL. Archives are streamed to disk (never fully loaded into memory), already-downloaded
files are skipped, and a `--limit N` flag caps how many files to fetch per run.

### Rust concepts introduced

**External crates (`Cargo.toml` dependencies)**
Rust's package manager Cargo resolves dependencies declared in `Cargo.toml`. Adding
`ureq = "3"` pulls the `ureq` HTTP client and all its transitive dependencies. We chose
`ureq` over `reqwest` because it's blocking-only with minimal dependencies — no hidden
async runtime. This keeps the code simple while we learn fundamentals.

**`Box<dyn Error>` — trait objects for error handling**
`Box<dyn Error>` is a heap-allocated pointer to *any* type implementing the `Error` trait.
This lets a function return different error types (io::Error, ureq::Error, etc.) through
a single return type. The caller doesn't need to know the concrete type — it can still
print the error message via the `Display` trait.

**The `?` operator — early return on error**
`?` is syntactic sugar for: "if this Result is Err, return the error from the current
function; otherwise, unwrap the Ok value." It replaces verbose `match` blocks and makes
error propagation concise. It also automatically converts error types when the function
returns `Box<dyn Error>`.

**`Path` / `PathBuf` — cross-platform file paths**
`Path` is a borrowed path slice (like `&str`), `PathBuf` is the owned version (like
`String`). `.join()` appends path components with the correct OS separator. Unlike raw
strings, these types handle platform differences (e.g., `/` vs `\`) correctly.

**`fs::create_dir_all` — idempotent directory creation**
Like `mkdir -p` in Unix: creates the directory and all parents if needed, silently
succeeds if the directory already exists. Idempotent operations like this are great for
robustness — the program works whether it's the first run or the hundredth.

**`File::create` vs `File::open`**
`File::open` opens a file for reading. `File::create` opens for writing, creating the
file if it doesn't exist or truncating it if it does. Two separate functions make the
intent explicit at the call site.

**`impl Trait` in function parameters**
`fn download_with_progress(reader: impl Read, writer: impl Write, ...)` accepts *any*
type implementing `Read` or `Write`. This is compile-time polymorphism — the compiler
generates specialized code for each concrete type used. It's Rust's equivalent of
generics with trait bounds, written in a more readable form.

**`Read` / `Write` traits — streaming I/O**
The `Read` trait provides `.read(&mut buf)` for pulling bytes from a source. The `Write`
trait provides `.write_all(&buf)` for pushing bytes to a destination. By programming
against traits (not concrete types), our download function works with files, network
streams, or in-memory buffers.

**`.take(n)` on iterators — lazy limiting**
`.take(n)` wraps an iterator so it yields at most `n` items, then stops. It's lazy —
it doesn't collect or allocate, just stops pulling from the inner iterator. We use it
to cap downloads: `.lines().filter(...).take(limit)`.

**`str::parse::<T>()` — parsing strings into typed values**
The turbofish syntax `::<usize>` tells the compiler what type to parse into. `.parse()`
returns `Result<T, ParseError>`, so invalid input is caught at runtime without panicking.

**Borrow checker and temporary lifetimes**
When chaining method calls like `.into_body().with_config().reader()`, each intermediate
value must live long enough for the next call to borrow from it. We split the chain into
separate `let` bindings so the borrow checker can verify that each reference is valid.
This is a common Rust pattern — explicit bindings make lifetimes clear.

### What's next

Step 3: Decompress the downloaded `.warc.gz` files and parse WARC records to extract
their content for further processing.
