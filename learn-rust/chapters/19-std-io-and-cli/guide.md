# 19 — std I/O, files, processes and CLI programs

> **Builds on:** chapters 06 (strings, FromStr), 07 (errors), 09 (generics over Read/Write), 10 (iterators).

**Why this matters.** Every program has an edge where bytes come in and go
out: files, standard input and output, sockets, other processes, environment
variables and exit codes. The databroker reads VSS JSON files, TLS
certificates and JWT keys from disk. It creates and removes a Unix socket,
takes its configuration from the environment, and exits with a status that
systemd or Docker acts on. `databroker-cli` is a full terminal program. This
chapter covers `std::io` and friends well enough to write small tools yourself
(and in this course, the devcontainer is Linux, so that's the platform these
examples assume). It ends with the Part II checkpoint.

---

## Read, BufRead and Write

Three traits carry almost all I/O in Rust:

| Trait | Key methods | Implemented by |
|---|---|---|
| `Read` | `read(&mut [u8]) -> io::Result<usize>`, `read_to_end`, `read_to_string` | `File`, `TcpStream`, `Stdin`, `&[u8]`, `Cursor<T>`, `ChildStdout` |
| `BufRead: Read` | `read_line`, `lines()`, `split(b)`, `fill_buf`/`consume` | `BufReader<R>`, `StdinLock`, `&[u8]`, `Cursor<T>` |
| `Write` | `write`, `write_all`, `flush`, and the `write!`/`writeln!` macros | `File`, `TcpStream`, `Stdout`, `Vec<u8>`, `BufWriter<W>`, `ChildStdin` |

Writing functions against the *traits* instead of concrete types is the most
useful I/O habit you can have:

```rust
fn parse_vss<R: std::io::Read>(reader: R) -> Result<Tree, Error> { ... }
```

In production that `R` is a `BufReader<File>`. In tests it's a `&[u8]` or
`Cursor<&str>` built from a string literal, with no files needed. This is
exactly how the databroker's VSS parser is written
(`databroker/src/vss.rs:533`), and `main.rs` passes it a buffered file
(`main.rs:159-161`).

The contract of `read` is subtle, and important:

- It returns `Ok(n)` with **n ≤ buf.len()**. A short read is normal, even
  on a file.
- `Ok(0)` means **end of input** (for a non-empty buffer).
- `Err(e)` with `e.kind() == ErrorKind::Interrupted` means a signal arrived.
  **Retry** it. `read_to_end`, `read_exact` and `io::copy` do this for you.
- Likewise, `write` may write only part of the buffer. Use `write_all`,
  which loops for you.

`io::Error` carries a **kind** (`NotFound`, `PermissionDenied`, `InvalidData`,
`UnexpectedEof`, `Interrupted`, …). Match on `e.kind()`, not on the message
text. `ErrorKind` is `#[non_exhaustive]`, so a `match` always needs a `_` arm.

**Buffering.** Every `read`/`write` on a raw `File` is a system call. Wrap
files and sockets in `BufReader`/`BufWriter` (8 KiB buffer by default) when you
do many small reads or writes. A `BufWriter` flushes when dropped, but
**errors during that final flush are silently ignored**, so call `flush()`
yourself when you care. `stdout()` is line-buffered, and inside a loop it's
faster to take the lock once with `let mut out = io::stdout().lock();`.

A neat detail: `impl<R: Read + ?Sized> Read for &mut R` (and the same for
`BufRead`/`Write`) means you can pass `&mut reader` to a function that takes
`R: Read` by value, then keep using the reader afterwards (c19_e01).

## Files and paths

**Paths are not strings.** On Linux a file name is any sequence of bytes
except `/` and NUL. It needn't be valid UTF-8. On Windows it's a sequence of
16-bit units. So the standard library has:

| Owned | Borrowed | Like |
|---|---|---|
| `PathBuf` | `&Path` | `String` / `&str`, for file system paths |
| `OsString` | `&OsStr` | `String` / `&str`, for OS strings (names, env values, args) |

Converting to text is explicit: `path.to_str()` returns `Option<&str>` (`None`
if not UTF-8), `path.display()` is for printing, and `to_string_lossy()`
replaces invalid bytes with `�`. Functions that open files take
`impl AsRef<Path>`, so `&str`, `String`, `&Path` and `PathBuf` all work.
`unlink_unix_domain_socket(path: impl AsRef<Path>)` in
`databroker/src/main.rs:223` is an example.

Useful `Path` methods: `join`, `parent`, `file_name`, `file_stem`,
`extension`, `with_extension`, `components`, `is_file`, `exists`. **Build
paths with `join`, never with `format!("{dir}/{name}")`.**

`std::fs` covers the everyday operations:

```rust
let text  = fs::read_to_string(&path)?;          // whole file → String (UTF-8 checked)
let bytes = fs::read(&path)?;                    // whole file → Vec<u8>
fs::write(&path, "contents")?;                   // create/truncate + write
fs::create_dir_all(dir)?;                        // like `mkdir -p`
for entry in fs::read_dir(dir)? { let path = entry?.path(); }  // no guaranteed order!
fs::rename(tmp, final_path)?;                    // atomic replace on the same file system
let meta = fs::metadata(&path)?;                 // size, type, permissions, times
```

For finer control there's `File::open`, `File::create` and `OpenOptions`
(`.append(true)`, `.create_new(true)`). The databroker uses
`OpenOptions::new().read(true).open(filename)` (`main.rs:159`).

**Atomic writes.** Writing a file in place can leave a half-written file if
the process dies mid-way. The standard pattern is to write to `name.tmp` in
the same directory, then `fs::rename` it over the target. On Linux (POSIX) a
rename within one file system replaces the target atomically: readers see the
old file or the new one, never a mix.

**Temporary files in tests.** Use a unique directory under
`std::env::temp_dir()` (that's `/tmp` in the devcontainer), for example
`learn-rust-<test>-<process id>-<counter>`, and remove it in a `Drop` impl.
Then it's cleaned up even when an assertion panics. The `tempfile` crate
does this properly in real projects.

## Processing input line by line

```rust
for (index, line) in reader.lines().enumerate() {
    let line = line?;            // io::Result<String>: I/O errors, including invalid UTF-8
    let line_no = index + 1;     // humans count from 1
    // ... parse `line`; record parse errors with `line_no` and keep going
}
```

- **Streaming keeps memory flat.** `lines()` reads a chunk at a time and
  yields one `String` per line, so a 10 GB log costs the same memory as a 10
  line one. `fs::read_to_string` loads everything, which is fine for config
  files but not for logs or data dumps.
- `lines()` allocates a new `String` per line. In a hot loop, reuse one
  buffer: `let mut buf = String::new(); while reader.read_line(&mut buf)? != 0 { ...; buf.clear(); }`.
- `lines()` strips `\n` *and* a preceding `\r`. `read_line` keeps them.
- Invalid UTF-8 is an `io::Error` with kind `InvalidData`. If the input may
  not be text, use `split(b'\n')` or `read_until`, which give you `Vec<u8>`.
- Distinguish **I/O errors** (stop, return `Err`) from **data errors** (a
  bad line: record it with its line number, carry on, report at the end).
  Chapter 17 is about exactly this decision.

## Processes, environment and exit codes

**Running other programs** uses `std::process::Command`, a builder:

```rust
let output = Command::new("git")
    .args(["log", "-1", "--format=%H"])
    .current_dir(repo)
    .env("GIT_PAGER", "cat")          // for this child only; the parent is unchanged
    .output()?;                       // run, wait, capture stdout + stderr
if !output.status.success() {
    eprintln!("git failed ({:?}): {}", output.status.code(),
              String::from_utf8_lossy(&output.stderr));
}
```

| Method | Stdout/stderr | Waits? |
|---|---|---|
| `.status()` | inherited (goes to your terminal) | yes |
| `.output()` | captured into `Output { status, stdout, stderr }` | yes |
| `.spawn()` | as configured with `.stdin/.stdout/.stderr(Stdio::…)` | no: returns a `Child` |

- A program that doesn't exist gives an `io::Error` of kind `NotFound` from
  `spawn`/`output`. That's different from a program that runs and *fails*
  (non-zero exit status).
- `status.code()` is `None` when the child was killed by a signal
  (Unix-only situation, e.g. `kill -9`).
- To feed a child's stdin: `.stdin(Stdio::piped())`, `child.stdin.take()`,
  `write_all`, then **drop the pipe** so the child sees end of input, then
  `wait_with_output()`. Forgetting the drop is a classic deadlock: the child
  waits for more input while you wait for the child.
- `Command` doesn't use a shell. `args` are passed exactly as given, so no
  quoting or injection problems. Only use `sh -c "..."` when you actually need
  shell features, and never build that string from untrusted input.

**Environment variables**: `std::env::var("KEY")` returns
`Result<String, VarError>`. It fails if the variable is missing *or* not
UTF-8; `var_os` gives an `Option<OsString>`. Two rules:

1. **Read the environment at the edge and pass values in.** A function like
   `port_from_env(lookup: impl Fn(&str) -> Option<String>)` can be tested
   with a `HashMap`, without touching the real process environment. The
   databroker has clap do this for it: every option has
   `.env("KUKSA_DATABROKER_...")` (`databroker/src/main.rs:286`).
2. **Don't call `std::env::set_var` in multi-threaded programs or tests.**
   Tests run on several threads and the environment is shared by the whole
   process, so a concurrent `getenv` can crash. In edition 2024
   `set_var` became `unsafe` for that reason. Use `Command::env` for children,
   or inject values as in rule 1.

`std::env::args()` gives the command-line arguments as `String`s (panicking on
non-UTF-8; `args_os()` doesn't). The first item is the program name.

**Exit codes** are how a process tells its caller (shell, systemd,
Docker, CI) what happened: 0 means success, anything else is failure. Common
conventions are `grep`'s 0/1/2 (match / no match / error) and the BSD
`sysexits` values (64 usage, 74 I/O error). The ways to set one:

| Approach | Effect |
|---|---|
| `fn main() -> Result<(), E>` where `E: Debug` | `Err` prints `Error: {e:?}` to stderr and exits with **1**. The databroker does this (`main.rs:232`). |
| `fn main() -> std::process::ExitCode` | You pick the code: `ExitCode::from(74)`, `ExitCode::SUCCESS`. |
| `std::process::exit(code)` | Exits **immediately**: no destructors run, so `BufWriter`s aren't flushed and temp files aren't cleaned up. Use it sparingly. |

## A testable CLI program

The key design for any command-line tool: **`main` is a thin shell.**

```rust
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args, io::stdin().lock(), io::stdout().lock(), io::stderr().lock());
    std::process::ExitCode::from(code)
}

fn run(args: &[String], stdin: impl BufRead, stdout: impl Write, stderr: impl Write) -> u8 {
    // parse args → open input → do the work → map outcomes to exit codes
}
```

Everything interesting is in `run`, which takes its inputs and outputs as
parameters, so tests call it with `Cursor`s and `Vec<u8>`s and check the
exact output and exit code. No subprocesses, no real terminal, no global
state. The same idea scales up: the databroker's `main` does little more than
parse arguments, build a `DataBroker` and call `grpc::server::serve_tcp`, so
most of its behaviour can be tested through the library crate.

Other habits of good CLI programs:

- Data goes to **stdout**, diagnostics go to **stderr**, so
  `tool | other-tool` works.
- Error messages are prefixed with the program name (`vss-grep: file.txt: No such file or directory`).
- Usage errors print the usage line and exit with a distinct code.
- Arguments are parsed by hand only for tiny tools. Real ones use `clap`
  (chapter 31), which `databroker-cli` uses (`databroker-cli/src/cli.rs:36`).

## Checkpoint

`c19_e06_checkpoint_part2` closes Part II. It gives you signatures, types
and tests but no step-by-step guidance. You build a small config-file
loader that uses:

- **chapter 06**: `FromStr` for a log level;
- **chapter 14**: a typestate builder whose `build()` exists only once a
  required value was supplied (`PhantomData`);
- **chapter 17**: a layered error type with `source()` chains, a boxed
  `dyn Error + Send + Sync` cause, downcasting, and rendering a chain;
- **chapter 18**: a `macro_rules!` macro that generates a struct *and* its
  `Default` impl from `field: Type = default` syntax;
- **this chapter**: reading a file and reporting I/O errors with the path.

If it takes a while, good: that's the point. If you're stuck for more than an
hour on one part, re-read the relevant chapter section rather than the
solution.

## Mental model

| Idea | Analogy |
|---|---|
| `Read` / `Write` | A hose: you don't care whether the other end is a tank (file), a pipe (process) or a well (socket) |
| `BufReader` | A bucket that fills from the hose in big scoops, so you can take one cup (line) at a time |
| `Path` vs `str` | A postal address vs a sentence: it may *look* like text but has its own rules |
| Atomic rename | Swapping the whole signpost at once instead of repainting it letter by letter |
| Exit code | The one-word answer a program gives its caller |
| Thin `main` | A plug: all the machinery is in the appliance, which you can test on the bench |

**Guarantees:** Rust doesn't make I/O infallible, but it makes failure
*visible*. Nearly every I/O operation returns `io::Result`, and ignoring it
triggers the `unused_must_use` warning. Buffers are bounds-checked, and
`Path` stops you from mixing up file names and text by accident.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`read(&mut buf)` fills the buffer." | It returns *some* bytes; loop, or use `read_exact`/`read_to_end`. |
| "`write` writes everything." | Use `write_all`. And `flush` before relying on the data being out. |
| "A file name is a `String`." | It's an `OsStr`: arbitrary bytes on Linux. `to_str()` can return `None`. |
| "`read_dir` returns files in alphabetical order." | The order is whatever the file system returns. Sort it yourself. |
| "`std::process::exit` is a normal return." | It skips destructors: buffers aren't flushed, `Drop` cleanups don't run. |
| "`Command::new("ls -l")` runs `ls` with `-l`." | That looks for a program literally named `ls -l`. Use `.arg("-l")`. |
| "Setting env vars in tests is harmless." | Tests run in parallel threads sharing one environment. Inject values instead. |
| "`fn main() -> Result` prints a nice error." | It prints the `Debug` form (`Error: Io { .. }`). Format it yourself for users. |

## Smells and idioms

| Smell | Idiomatic form |
|---|---|
| A function taking `&File` or `&str` file names deep inside the logic | Take `impl Read`/`impl BufRead`/`impl Write`, and open files at the edge |
| `fs::read_to_string` on unbounded input | Stream with `BufReader::lines()` |
| `format!("{}/{}", dir, name)` | `dir.join(name)` |
| `path.to_str().unwrap()` | Handle non-UTF-8 paths, or use `display()` for messages |
| Matching on `err.to_string()` | Match on `err.kind()` |
| `unwrap()` on stdout writes in a CLI | Handle the error: a closed pipe (`| head`) is `BrokenPipe`, which isn't a crash-worthy bug |
| `std::process::exit` deep in library code | Return an error; let `main` decide the exit code |
| `databroker-cli` prints an error and still exits with **0** (`databroker-cli/src/main.rs:24-28`: `Err(e) => eprintln!("Error: {e}")`) | Return `ExitCode::FAILURE` (or `Result` from `main`) so scripts can detect failure. A good first contribution. |
| `sh -c` with interpolated user input | `Command::new(prog).args(...)`: no shell, no injection |

## In the databroker and in the wild

In the databroker:

| Where | What to notice |
|---|---|
| `databroker/src/vss.rs:533` | `parse_vss_from_reader<R: std::io::Read>`: generic over the source, tested with `parse_vss_from_str` (line 542). |
| `databroker/src/main.rs:153-161` | `read_metadata_file`: `OpenOptions` → `BufReader` → the generic parser; `?` converts both `io::Error` and `vss::Error` into `Box<dyn Error>`. |
| `databroker/src/main.rs:223-230` | `unlink_unix_domain_socket(path: impl AsRef<Path>)`: `fs::metadata`, `file_type().is_socket()` (a Unix-only extension trait, `std::os::unix::fs::FileTypeExt`), then `remove_file`. |
| `databroker/src/main.rs:592` | `create_dir_all` on the socket's parent directory. Note the `.unwrap()` on `parent()`: a path of `/` would panic. |
| `databroker/src/main.rs:232` | `fn main() -> Result<(), Box<dyn std::error::Error>>`: an `Err` exits with code 1 and a `Debug`-printed message. |
| `databroker/src/main.rs:286` | `.env("KUKSA_DATABROKER_PORT")`: configuration from the environment, through clap. |
| `databroker-cli/src/kuksa_cli.rs:265` | `std::fs::read_to_string(token_filename)?` to load a JWT token. |
| `databroker-cli/src/kuksa_cli.rs:347` | The interactive loop reads one line at a time from the terminal. |
| `databroker-cli/src/main.rs:21-31` | `#[tokio::main] async fn main()` that prints errors but exits 0 (see Smells). |

In the wild (std sources ship with rustup: run
`rustc --print sysroot` and look under `lib/rustlib/src/rust/library/std/src/`;
line numbers are for Rust 1.96):

| File | What to read |
|---|---|
| `std/src/io/mod.rs:714` (`trait Read`), `:1699` (`trait Write`), `:2330` (`trait BufRead`) | The doc comments on each method spell out the exact contracts: short reads, `Interrupted`, EOF. |
| `std/src/io/mod.rs:2609` (`read_line`), `:2684` (`lines`) | How `lines()` is built on `read_line`, and where the `\r\n` stripping happens. |
| `std/src/io/copy.rs:62` | `io::copy`: the production version of your `copy_uppercase` loop. |
| `std/src/io/buffered/bufreader.rs:51` | `BufReader` and its default capacity. |
| `std/src/process.rs:596` (`Command`), `:1083` (`output`) | The builder, and how `output` wires up the pipes. |
| `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/serde_json-1.0.151/src/de.rs:2624` | `serde_json::from_reader`, which the databroker's VSS parser calls. Note the doc advice to pass a `BufReader`. |

## Exercises

| Exercise | Goal |
|---|---|
| c19_e01_read_write_traits | Generic I/O over `Read`/`BufRead`/`Write`: chunked copying, `Interrupted`, line reading, aligned output |
| c19_e02_files_and_paths | `Path`/`OsStr`, `read_dir`, backup names, atomic writes, RAII temp dirs |
| c19_e03_line_processing | Stream 100 000 lines with constant memory, with line-numbered data errors and I/O errors |
| c19_e04_process_and_env | `Command`: capture, exit status, stdin pipes, per-child env; testable env config; exit codes |
| c19_e05_cli_program_end_to_end | `vss-grep`: argument parsing, input selection, grep-style exit codes, all tested without a terminal |
| c19_e06_checkpoint_part2 | **Checkpoint**: config loader with a macro, `FromStr`, a typestate builder and layered errors |

## Further reading

- The Rust Book, chapter 12 "An I/O Project: Building a Command Line Program": <https://doc.rust-lang.org/book/ch12-00-an-io-project.html>
- `std::io` module docs (read the overview): <https://doc.rust-lang.org/std/io/>
- `std::path`, `std::fs`, `std::process`, `std::env` docs on doc.rust-lang.org
- *Command Line Applications in Rust*: <https://rust-cli.github.io/book/>
- Rust by Example, "Std misc → File I/O" and "Child processes": <https://doc.rust-lang.org/rust-by-example/std_misc.html>

## Check yourself

1. `reader.read(&mut buf)` returned `Ok(3)` for a 16-byte buffer. What does that tell you, and what should your loop do?
2. Why should a parsing function take `impl BufRead` rather than a file name?
3. Why is `Path::to_str` fallible?
4. How do you replace a file so that readers never see a half-written version?
5. You spawn `tr a-z A-Z` with piped stdin, write to it, and call `wait_with_output()`, but it hangs. Why?
6. Why is `std::env::set_var` risky in tests, and what should you do instead?
7. What's the difference between returning `Err` from `main` and calling `std::process::exit(1)`?
8. What exit code does `databroker-cli` return after printing "Error: ..."? Why is that a problem?

<details><summary>Answers</summary>

1. Only 3 bytes were available right now. That isn't end of input (that would
   be `Ok(0)`), so process those 3 bytes and call `read` again.
2. The caller can pass a file, stdin, a socket, or an in-memory `Cursor` in
   tests. The function does one job and opening files stays at the edge.
3. File names are OS strings (arbitrary bytes on Linux), which may not be valid UTF-8.
4. Write to a temporary file in the same directory, then `fs::rename` it over
   the target. A rename within one file system is atomic.
5. The child's stdin was never closed, so `tr` keeps waiting for more input
   while you wait for it to exit. Take the pipe out of the `Child` and drop it
   after writing.
6. The environment is shared by the whole process and tests run on parallel
   threads, so concurrent `set_var`/`getenv` can race or crash (edition 2024
   marks `set_var` `unsafe`). Inject a lookup function, or use `Command::env`
   for child processes.
7. Returning `Err` unwinds normally: destructors run (buffers flush, temp
   files are cleaned up), then it prints `Error: {e:?}` and exits with 1.
   `process::exit` stops immediately without running destructors.
8. 0 (success), because `main` just prints the error and returns normally. Scripts
   and CI can't tell that it failed.
</details>
