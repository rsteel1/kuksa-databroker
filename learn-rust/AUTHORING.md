# Authoring contract for the kuksa-databroker Rust course

This file is the single source of truth for everyone writing chapters.
The learner is a **complete Rust beginner**. The goal is a *complete*
understanding of Rust, deep enough to read, debug and contribute to **any**
Rust codebase, with `kuksa-databroker` (repo root = `..`) as the running
example and capstone. See CURRICULUM.md for the chapter order.

**Prerequisite rule:** a chapter may only rely on concepts taught in
earlier chapters. If you need a later concept, explain it in two lines
where you use it, or restructure the exercise. For example, before
chapter 20 use `std::thread::spawn`, not `tokio::spawn`, to illustrate
`'static` bounds.

## Layout

```
learn-rust/
  chapters/NN-slug/                 one folder per chapter (NN = two digits, same as cNN)
    guide.md                        the chapter text
    hints.md                        `## cNN_eMM_slug` section per exercise
    exercises/cNN_eMM_slug.rs       one exercise per file (one test target each)
    solutions/cNN_eMM_slug.rs       same name, finished version
  tooling/exercises, tooling/solutions   Cargo packages that compile the chapter files
  tooling/rl/                       the runner (`cargo rl ...`)
```

Cargo can't auto-discover test files outside a package's `tests/` folder,
so `tooling/{exercises,solutions}/Cargo.toml` list every file as a
`[[test]]` target. After adding, renaming or removing an exercise, run
`cargo rl sync-manifests`. `check-solutions` fails if the lists are stale.

## Exercise file rules

1. Header comment, in this shape:

   ```rust
   // cNN_eMM_slug.rs
   //
   // CONCEPT: <one line>
   // GUIDE:   chapters/NN-slug/guide.md  (section "<exact section title>")
   // DATABROKER: databroker/src/<file>.rs — <what to look at, fn/struct name>
   // EXPECT:  <what you'll see first, e.g. "compile error E0382 (use of moved value)"
   //           or "tests fail: todo!() panics">
   //
   // <2-10 lines of explanation + motivation from the databroker>
   //
   // YOUR TASK
   //   <precise instructions; say which code may be edited>
   //   Do not edit the tests.

   // I AM NOT DONE
   ```
   The marker line must be exactly `// I AM NOT DONE` on its own line.
2. Tests at the bottom in `#[cfg(test)] mod tests { use super::*; ... }`.
   Async tests: `#[tokio::test]`; time-dependent tests use
   `#[tokio::test(start_paused = true)]` so they're deterministic and fast.
   **No real sleeps > 50 ms. No network. No flaky timing.**
3. As shipped the exercise must FAIL (compile error or failing test or `todo!()`).
   Prefer, in this order by chapter stage:
   - early chapters: one focused compile error that teaches a rule (name the
     error code in EXPECT so the learner can run `rustc --explain`);
   - later chapters: `todo!()` bodies with real logic to write, plus tests
     that pin behaviour down precisely.
4. The fix should be *learnable*: exercises in a chapter ramp from tiny to
   genuinely challenging. The last exercise of each chapter is a
   "databroker-shaped" task that mirrors real code from the repo (simplified,
   self-contained — rename types if needed, but keep the shape recognisable
   and cite the real file in DATABROKER).
5. Only use crates in `tooling/exercises/Cargo.toml`: std, tokio (full + test-util),
   tokio-stream, futures, async-trait, async-stream, serde, serde_json, regex,
   glob-match, indexmap, uuid, thiserror, anyhow, clap, tracing,
   tracing-subscriber, tonic, prost, prost-types, tower (util), bytes,
   pin-project-lite, proptest, databroker, databroker-proto, and `macros`
   (chapter 18's proc-macro crate: `chapters/18-macros/exercise-macros/` for
   exercises and `chapters/18-macros/solution-macros/` for solutions, both imported as `macros`). `syn`,
   `quote` and `proc-macro2` are only available inside those two crates.
   `databroker` is built with default features off (no `tls`, no `viss`, no `otel`).
   No network access except loopback (`127.0.0.1`, port 0) in chapter 37.
6. `unsafe_code` is `deny` crate-wide. Only chapters 26 and 27 may opt in,
   per file, with `#![allow(unsafe_code)]` as the first line after the
   header. Every `unsafe` block in a solution needs a `// SAFETY:` comment
   (clippy `undocumented_unsafe_blocks` is on for solutions).
   `dead_code` is allowed crate-wide. Solutions must be **clippy clean with
   `-D warnings`** and idiomatic (see Style). Exercises may contain the
   un-idiomatic code the learner must fix, but not accidental clippy noise.
7. The solution file = exercise file with the marker line (and the blank line
   after it) removed and the task done. Keep the header comment identical.
   Solutions may add short `// WHY:` comments explaining the key insight.

## Checkpoints

The last exercise of chapters 13, 19, 24, 27 and 32 is a checkpoint:
`cNN_eMM_checkpoint_partX`. A checkpoint is a mixed-topic problem with
minimal scaffolding (signatures and tests only). Its GUIDE line points at a
`## Checkpoint` section in that chapter's guide, which lists the chapters it
draws on without giving the solution. The hint gives strategy, not code.

## Hints (`chapters/NN-slug/hints.md`)

```markdown
# Chapter NN hints

## cNN_eMM_slug
Progressive hints: first a nudge, then the concept, then (last paragraph)
nearly the answer. Name error codes and `rustc --explain` where relevant.
```

## Guide chapter (`chapters/NN-slug/guide.md`) structure

Write for a smart beginner. Explain *why*, not just *what*. Use small code
snippets. Required sections, in order:

1. `# NN — Title` and a one-paragraph "Why this matters for the databroker".
2. **Concept sections** (one `##` per idea; exercise GUIDE lines point at these titles).
3. `## Mental model` — an analogy table and the guarantee the compiler gives
   (e.g. "no data races", "no use-after-free").
4. `## Common misconceptions` — table: wrong model → correct model, with error codes.
5. `## Smells and idioms` — anti-patterns to avoid and the idiomatic form
   (clone-everywhere, unwrap in library code, `String` where `&str` fits,
   index loops, holding locks across `.await`, `get_` prefixes, etc.).
   Where the databroker itself deviates from idiom, say so honestly and
   explain the trade-off (it's a real codebase, not a textbook).
   "In the wild" references point at crate sources already on disk under
   `~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/`. Take the
   exact versions from `learn-rust/Cargo.lock` and verify the paths exist.
6. `## In the databroker` (general-Rust chapters: `## In the databroker and in the wild`)
   — a guided reading list of real code with
   `path:line` references (verify line numbers with grep — they must be
   correct) and what to notice in each. Quote short snippets.
7. `## Exercises` — table of this chapter's exercises with one-line goals.
8. `## Further reading` — Rust Book / Rust by Example / Tokio tutorial /
   crate docs sections (name the section; URLs to doc.rust-lang.org,
   tokio.rs, docs.rs are fine).
9. `## Check yourself` — 4-8 questions (answers in a `<details>` block).

## Style (solutions and snippets)

- rustfmt formatting. snake_case fns/vars, CamelCase types, SCREAMING consts.
- No `get_` prefix on our own getters (`fn id()` not `fn get_id()`).
- `?` for propagation; `expect("why this can't fail")` over `unwrap()` outside tests.
- Iterators over index loops; `&str` params over `&String`; `impl AsRef<str>` /
  `impl Into<String>` where the databroker uses them.
- Don't hold a lock guard across `.await`; scope guards tightly; snapshot then await.
- `tokio::time::sleep`, never `std::thread::sleep`, inside async.
- Prefer `std::sync::OnceLock`/`LazyLock` over `lazy_static` in new code
  (and explain the databroker still uses `lazy_static`).
- Edition is 2021 to match the databroker. Mention edition-2024 differences
  only where they matter (e.g. let-chains, RPIT lifetime capture).

## Verifying your work (mandatory before you finish)

Run only per-target commands (other authors are working in parallel; never
run a workspace-wide `cargo build --tests` because other people's exercises
are intentionally broken):

```sh
cd learn-rust
cargo rl check-solutions cNN      # per chapter prefix; must be all ✓
rustfmt --edition 2021 --check chapters/NN-*/exercises/*.rs chapters/NN-*/solutions/*.rs
```

`check-solutions` verifies for each exercise: the solution passes tests, the
solution is clippy clean with -D warnings, the exercise still has the marker
and does NOT pass as shipped, and a hint section exists.

Also double-check every `path:line` reference in your guide chapter with grep.
Do not touch files outside your assigned chapters (no edits to Cargo.toml,
rl/, README.md, CURRICULUM.md or other chapters). When adding exercises to an
existing chapter, append to its hints file and add sections to its guide
without rewriting what's there. If you believe a new crate is needed, don't
add it — work within the list above and mention it in your report.
