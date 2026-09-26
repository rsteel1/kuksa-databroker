# Rust from zero to mastery (with kuksa-databroker)

A rustlings-style course that takes you from never having written Rust to
reading, debugging and contributing to **any** Rust codebase. **This
repository's databroker** runs through the whole course as the example and
the final project. Every chapter teaches a concept, shows where the
databroker uses it (with real `file:line` references), and in the
general-Rust chapters also points at well-known crates like tokio, serde
and anyhow.

```
Part I   Foundations        01 basics → 13 threads          (+ checkpoint)
Part II  Intermediate       14 advanced traits → 19 std I/O & CLI programs
                            (incl. 17 error handling mastery, 18 macros) (+ checkpoint)
Part III Async              20 async basics → 24 async internals (+ checkpoint)
Part IV  Systems            25 concurrency in depth, 26 unsafe & FFI,
                            27 performance                   (+ checkpoint)
Part V   Real-world domains 28 serde → 32 testing, 33 Cargo & API design,
                            34 reading any Rust codebase     (+ checkpoint)
Part VI  Capstones          35 build a mini-databroker, 36 drive the real one,
                            37 projects with no scaffolding
         Contributing       chapter 99
```

- [CURRICULUM.md](CURRICULUM.md) lists every chapter and exercise.
- [MASTERY.md](MASTERY.md) maps the standard Rust references (the Book, the
  Async Book, the Rustonomicon, *Rust for Rustaceans*, the API Guidelines) to
  the chapter that covers each topic, plus a self-assessment checklist.

## Getting started

1. **Open the dev container.** Everything you need (Rust 1.96 with clippy and
   rustfmt, rust-analyzer, the C/C++ toolchain the build needs, and the
   `proto/` submodule) is set up in a Linux container:
   - Install Docker and VS Code with the **Dev Containers** extension.
   - Open the repository folder in VS Code and run **"Dev Containers: Reopen
     in Container"**, then pick **"Learn Rust (kuksa-databroker course)"**.
   - The first start builds the image and pre-compiles all dependencies,
     which takes several minutes once. After that, containers start in seconds.

   You can also use the CLI (`devcontainer up --workspace-folder .
   --config .devcontainer/learn-rust/devcontainer.json`) or GitHub
   Codespaces. Every command in this course runs **inside the container**.
2. **Read [chapters/00-setup-and-overview/guide.md](chapters/00-setup-and-overview/guide.md)**
   first. It explains the tools, how to read compiler errors, and what the
   databroker is.
3. In the container's terminal, run:

   ```sh
   cd learn-rust
   cargo rl
   ```

   The first run compiles the dependencies (a few minutes, once). After that
   it shows you the first unfinished exercise and why it fails.

## The loop

```
cargo rl            → shows the current exercise and its error
edit the file       → chapters/NN-*/exercises/cNN_eMM_name.rs
cargo rl            → still failing? read the error again, then the guide
                      passing? delete the `// I AM NOT DONE` line
cargo rl            → moves on to the next one
```

`cargo rl watch` does the same thing automatically every time you save.

| Command | What it does |
|---|---|
| `cargo rl` | Run the next unfinished exercise |
| `cargo rl watch` | Re-run on every save (Ctrl-C to stop) |
| `cargo rl list` | Every exercise and where you are |
| `cargo rl hint` | Hint for the current exercise (`cargo rl hint c04_e02` for a specific one) |
| `cargo rl run c04_e02` | Run a specific exercise (a unique prefix is enough) |
| `cargo rl solution` | Show where the solution file is |
| `cargo rl verify` | Re-check every exercise from scratch |

## How to study (please read)

- **Read the guide chapter before its exercises.** Each exercise's header
  names the guide section it relies on.
- **Read the compiler error slowly, the whole thing.** Rust's errors are the
  best teacher you have. Every error code has a long explanation:
  `rustc --explain E0382`.
- **Hints are progressive.** The first paragraph is a nudge and the last one
  nearly gives the answer. Stop reading once you're unstuck.
- **Solutions are in each chapter's `solutions/` folder.** Look only after the exercise
  passes, to compare approaches, or after a real struggle.
- **Do the "In the databroker" reading.** Opening the real file and finding
  the pattern you just practised is what turns exercises into understanding
  of the codebase.
- Each chapter ends with **Check yourself** questions. If you can't answer
  them, re-read before moving on.
- Expect roughly 100–150 hours in total. Don't rush chapters 02 (ownership),
  11 and 16 (lifetimes), 17 (errors) or 20–24 (async).
- **Checkpoints** (the last exercise of chapters 13, 19, 24, 27 and 32) mix
  topics and give almost no scaffolding. If a checkpoint feels impossible,
  go back to the chapters it lists before reading its hint.

## Layout

```
learn-rust/
  README.md            this file
  CURRICULUM.md        chapter-by-chapter map
  MASTERY.md           what "knowing Rust" covers, and where each part is taught
  AUTHORING.md         conventions for anyone adding exercises
  chapters/
    00-setup-and-overview/guide.md
    01-basics/
      guide.md         the chapter text: read this first
      exercises/       your exercises (edit these)
      hints.md         progressive hints, one section per exercise
      solutions/       reference solutions
    …
    18-macros/
      exercise-macros/ the procedural-macro crate you edit in exercise c18_e06
      solution-macros/ its reference solution
    …
    99-capstone-contributing/guide.md
  tooling/             the Cargo packages that compile the chapter files, and
                       `rl`, the runner behind `cargo rl`. You won't need to edit these.
```

This directory is its own Cargo workspace. It doesn't change how the
databroker builds. The exercises depend on the real `databroker` and
`databroker-proto` crates through path dependencies, so the capstone
chapters use the code in this repository.

## The dev container

- **Configuration:** `.devcontainer/learn-rust/` holds the Dockerfile,
  `devcontainer.json` and `post-create.sh`. The repository's main
  `.devcontainer/` is the project's full release environment (cross
  compilation, SBOM tools, Docker-in-Docker). You don't need it for the course.
- **Build output:** kept in the named Docker volumes `kuksa-learn-rust-target`
  and `kuksa-databroker-target`, which is much faster than a bind mount on
  macOS and Windows hosts. To free the space, run `docker volume rm` on them
  after removing the container.
- **Toolchain:** pinned in `learn-rust/rust-toolchain.toml`, so compiler
  messages and clippy lints match what the exercises were checked against.
- **rust-analyzer:** configured (`rust-analyzer.linkedProjects`) to understand
  both this course's workspace and the databroker's, so go-to-definition works
  from your exercises into the real databroker code.
