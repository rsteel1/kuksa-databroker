//! `rl` — a tiny rustlings-style runner.
//!
//! Usage (from anywhere inside `learn-rust/`):
//!
//!   cargo rl                 run the next unfinished exercise (same as `cargo rl next`)
//!   cargo rl watch           like `next`, but re-runs every time you save the file
//!   cargo rl list            show every exercise and its status
//!   cargo rl run <name>      run one exercise (name can be a prefix, e.g. c04_e02)
//!   cargo rl hint [name]     show the hint for an exercise (default: current one)
//!   cargo rl solution [name] print where the solution file is
//!   cargo rl verify          re-run every exercise from scratch
//!   cargo rl reset-progress  forget cached results
//!
//! Course-author commands:
//!   cargo rl check-solutions [prefix]  every solution passes, every exercise
//!                                      still has its marker and fails as shipped
//!   cargo rl sync-manifests            regenerate the [[test]] lists in
//!                                      tooling/{exercises,solutions}/Cargo.toml
//!
//! Layout: chapters/NN-slug/{guide.md, hints.md, exercises/*.rs, solutions/*.rs}
//!
//! An exercise is *done* when its tests pass AND the line
//! `// I AM NOT DONE` has been deleted from the file.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime};

const MARKER: &str = "// I AM NOT DONE";

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

struct Course {
    root: PathBuf,
}

#[derive(PartialEq)]
enum Outcome {
    /// Compiles and all tests pass.
    Pass,
    /// Compile error or failing test. Holds cargo's output.
    Fail(String),
}

impl Course {
    fn new() -> Self {
        // tooling/rl -> learn-rust
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("rl lives in learn-rust/tooling/rl")
            .to_path_buf();
        Course { root }
    }

    /// The chapter folder for a two-digit chapter number, e.g. "04" -> chapters/04-enums.
    fn chapter_dir(&self, chapter: &str) -> Option<PathBuf> {
        let prefix = format!("{chapter}-");
        let mut dirs: Vec<PathBuf> = fs::read_dir(self.root.join("chapters"))
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .is_some_and(|f| f.to_string_lossy().starts_with(&prefix))
            })
            .collect();
        dirs.sort();
        dirs.into_iter().next()
    }

    /// Every `chapters/*/<kind>/*.rs` file, sorted by file name (= course order).
    fn files_in(&self, kind: &str) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = fs::read_dir(self.root.join("chapters"))
            .expect("chapters/ directory exists")
            .filter_map(|e| e.ok())
            .filter_map(|e| fs::read_dir(e.path().join(kind)).ok())
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort_by_key(|p| p.file_name().map(|f| f.to_os_string()));
        files
    }

    /// All exercise names (file stems) in course order.
    fn exercises(&self) -> Vec<String> {
        self.files_in("exercises")
            .iter()
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect()
    }

    fn file_for(&self, kind: &str, name: &str) -> PathBuf {
        let file = format!("{name}.rs");
        match name.get(1..3).and_then(|c| self.chapter_dir(c)) {
            Some(dir) => dir.join(kind).join(file),
            None => self.root.join("chapters").join(kind).join(file),
        }
    }

    fn exercise_path(&self, name: &str) -> PathBuf {
        self.file_for("exercises", name)
    }

    fn solution_path(&self, name: &str) -> PathBuf {
        self.file_for("solutions", name)
    }

    fn resolve(&self, prefix: &str) -> Option<String> {
        let all = self.exercises();
        if let Some(exact) = all.iter().find(|n| n.as_str() == prefix) {
            return Some(exact.clone());
        }
        let matches: Vec<&String> = all.iter().filter(|n| n.starts_with(prefix)).collect();
        match matches.as_slice() {
            [one] => Some((*one).clone()),
            [] => {
                eprintln!("{RED}No exercise matches `{prefix}`.{RESET} Try `cargo rl list`.");
                None
            }
            many => {
                eprintln!("{YELLOW}`{prefix}` is ambiguous:{RESET}");
                for m in many {
                    eprintln!("  {m}");
                }
                None
            }
        }
    }

    fn has_marker(&self, name: &str) -> bool {
        fs::read_to_string(self.exercise_path(name))
            .map(|s| s.lines().any(|l| l.trim() == MARKER))
            .unwrap_or(false)
    }

    fn cargo_test(&self, package: &str, name: &str) -> Outcome {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let output = Command::new(cargo)
            .args(["test", "-q", "-p", package, "--test", name, "--color", "always"])
            .current_dir(&self.root)
            .output()
            .expect("failed to launch cargo");
        let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stdout));
        if output.status.success() {
            Outcome::Pass
        } else {
            Outcome::Fail(text)
        }
    }

    fn cargo_clippy_solution(&self, name: &str) -> Outcome {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let output = Command::new(cargo)
            .args(["clippy", "-q", "-p", "solutions", "--test", name, "--", "-D", "warnings"])
            .current_dir(&self.root)
            .output()
            .expect("failed to launch cargo");
        if output.status.success() {
            Outcome::Pass
        } else {
            Outcome::Fail(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    // ---- progress cache -------------------------------------------------
    // `.progress` stores "<name> <hash-of-file>" for exercises that passed
    // with the marker removed, so `cargo rl` doesn't recompile everything.

    fn progress_file(&self) -> PathBuf {
        self.root.join(".progress")
    }

    fn load_progress(&self) -> HashMap<String, u64> {
        fs::read_to_string(self.progress_file())
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let (name, hash) = line.split_once(' ')?;
                Some((name.to_string(), hash.parse().ok()?))
            })
            .collect()
    }

    fn save_progress(&self, progress: &HashMap<String, u64>) {
        let mut lines: Vec<String> = progress.iter().map(|(n, h)| format!("{n} {h}")).collect();
        lines.sort();
        let _ = fs::write(self.progress_file(), lines.join("\n") + "\n");
    }

    fn file_hash(&self, name: &str) -> u64 {
        let content = fs::read(self.exercise_path(name)).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn is_cached_done(&self, progress: &HashMap<String, u64>, name: &str) -> bool {
        progress.get(name) == Some(&self.file_hash(name))
    }

    // ---- helpers for display --------------------------------------------

    fn guide_for(&self, name: &str) -> Option<PathBuf> {
        // c04_e02_xxx -> chapters/04-*/guide.md
        let guide = self.chapter_dir(name.get(1..3)?)?.join("guide.md");
        guide.exists().then_some(guide)
    }

    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    fn hint(&self, name: &str) -> Option<String> {
        let text =
            fs::read_to_string(self.chapter_dir(name.get(1..3)?)?.join("hints.md")).ok()?;
        let header = format!("## {name}");
        let start = text.find(&header)? + header.len();
        let rest = &text[start..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }

    fn progress_bar(&self, done: usize, total: usize) -> String {
        let width = 40;
        let filled = (done * width).checked_div(total).unwrap_or(0);
        format!(
            "[{GREEN}{}{RESET}{}] {done}/{total}",
            "#".repeat(filled),
            "-".repeat(width - filled)
        )
    }

    // ---- commands ---------------------------------------------------------

    /// Runs one exercise and prints a rustlings-style report.
    /// Returns true when the exercise is fully done.
    fn run_one(&self, name: &str, progress: &mut HashMap<String, u64>) -> bool {
        println!("{DIM}Running {name} ...{RESET}");
        let outcome = self.cargo_test("exercises", name);
        let marker = self.has_marker(name);
        let path = self.rel(&self.exercise_path(name));
        match (&outcome, marker) {
            (Outcome::Fail(out), _) => {
                println!("{out}");
                println!("{RED}{BOLD}✗ {name} does not pass yet.{RESET}");
                println!("  Edit:  {path}");
                if let Some(g) = self.guide_for(name) {
                    println!("  Read:  {}", self.rel(&g));
                }
                println!("  Stuck? cargo rl hint {name}");
                progress.remove(name);
                false
            }
            (Outcome::Pass, true) => {
                println!("{GREEN}✓ {name} compiles and its tests pass!{RESET}");
                println!(
                    "{YELLOW}  When you understand why, delete the `{MARKER}` line in {path}"
                );
                println!("  and run `cargo rl` again to move on.{RESET}");
                progress.remove(name);
                false
            }
            (Outcome::Pass, false) => {
                println!("{GREEN}{BOLD}✓ {name} done.{RESET}");
                progress.insert(name.to_string(), self.file_hash(name));
                true
            }
        }
    }

    /// Finds the first exercise that is not done. Uses the cache for speed.
    fn first_unfinished(&self, progress: &HashMap<String, u64>) -> Option<String> {
        self.exercises()
            .into_iter()
            .find(|name| self.has_marker(name) || !self.is_cached_done(progress, name))
    }

    fn next(&self) -> bool {
        let mut progress = self.load_progress();
        let total = self.exercises().len();
        loop {
            let Some(name) = self.first_unfinished(&progress) else {
                println!("{GREEN}{BOLD}🎉 Every exercise is done. You're ready for the databroker!{RESET}");
                println!("Next: chapters/99-capstone-contributing/guide.md");
                return true;
            };
            let done = self.run_one(&name, &mut progress);
            self.save_progress(&progress);
            if !done {
                let finished = self
                    .exercises()
                    .iter()
                    .filter(|n| self.is_cached_done(&progress, n))
                    .count();
                println!("\n{}", self.progress_bar(finished, total));
                return false;
            }
            // Exercise just completed: celebrate chapter changes, then continue.
            if let Some(following) = self.first_unfinished(&progress) {
                if following.get(0..3) != name.get(0..3) {
                    println!("{BOLD}Chapter {} complete!{RESET}", &name[1..3]);
                    if let Some(g) = self.guide_for(&following) {
                        println!("Before continuing, read {}", self.rel(&g));
                    }
                    return true;
                }
            }
        }
    }

    fn watch(&self) {
        println!("{DIM}Watching exercises/ — save a file to re-run. Ctrl-C to quit.{RESET}");
        let mut last_seen = self.newest_mtime();
        self.next();
        loop {
            std::thread::sleep(Duration::from_millis(700));
            let now = self.newest_mtime();
            if now > last_seen {
                last_seen = now;
                print!("\x1b[2J\x1b[H"); // clear screen
                self.next();
            }
        }
    }

    fn newest_mtime(&self) -> SystemTime {
        self.files_in("exercises")
            .iter()
            .filter_map(|p| fs::metadata(p).ok()?.modified().ok())
            .max()
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    // ---- Cargo manifests ------------------------------------------------
    // Cargo only auto-discovers tests in a package's own tests/ folder, so
    // tooling/{exercises,solutions}/Cargo.toml list every chapter file as an
    // explicit [[test]] target below this marker line.

    const MANIFEST_MARKER: &'static str =
        "# ---- generated by `cargo rl sync-manifests`: do not edit below ----";

    fn manifest_path(&self, package: &str) -> PathBuf {
        self.root.join("tooling").join(package).join("Cargo.toml")
    }

    /// The manifest content `sync-manifests` would write for a package.
    fn expected_manifest(&self, package: &str) -> Option<String> {
        let current = fs::read_to_string(self.manifest_path(package)).ok()?;
        let head = match current.find(Self::MANIFEST_MARKER) {
            Some(i) => current[..i].trim_end().to_string(),
            None => current.trim_end().to_string(),
        };
        let mut out = format!("{head}\n\n{}\n", Self::MANIFEST_MARKER);
        for path in self.files_in(package) {
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let rel = path.strip_prefix(&self.root).ok()?.display().to_string();
            out.push_str(&format!(
                "\n[[test]]\nname = \"{stem}\"\npath = \"../../{rel}\"\n"
            ));
        }
        Some(out)
    }

    fn manifests_in_sync(&self) -> bool {
        ["exercises", "solutions"].iter().all(|pkg| {
            fs::read_to_string(self.manifest_path(pkg)).ok() == self.expected_manifest(pkg)
        })
    }

    fn sync_manifests(&self) -> bool {
        for pkg in ["exercises", "solutions"] {
            let Some(content) = self.expected_manifest(pkg) else {
                eprintln!("{RED}Could not read tooling/{pkg}/Cargo.toml{RESET}");
                return false;
            };
            if fs::write(self.manifest_path(pkg), content).is_err() {
                eprintln!("{RED}Could not write tooling/{pkg}/Cargo.toml{RESET}");
                return false;
            }
            println!("{GREEN}✓{RESET} tooling/{pkg}/Cargo.toml ({} tests)", self.files_in(pkg).len());
        }
        true
    }

    fn list(&self) {
        let progress = self.load_progress();
        let names = self.exercises();
        let mut done = 0;
        let mut current_chapter = String::new();
        let mut marked_current = false;
        for name in &names {
            let chapter = name.get(0..3).unwrap_or("").to_string();
            if chapter != current_chapter {
                let guide = self
                    .guide_for(name)
                    .map(|g| self.rel(&g))
                    .unwrap_or_default();
                println!("\n{BOLD}{chapter}{RESET}  {DIM}{guide}{RESET}");
                current_chapter = chapter;
            }
            let is_done = !self.has_marker(name) && self.is_cached_done(&progress, name);
            if is_done {
                done += 1;
                println!("  {GREEN}✓{RESET} {name}");
            } else if !marked_current {
                marked_current = true;
                println!("  {YELLOW}➜{RESET} {BOLD}{name}{RESET}   {YELLOW}<- you are here{RESET}");
            } else {
                println!("  {DIM}· {name}{RESET}");
            }
        }
        println!("\n{}", self.progress_bar(done, names.len()));
    }

    fn verify(&self) -> bool {
        let mut progress = HashMap::new();
        let mut all_ok = true;
        for name in self.exercises() {
            let outcome = self.cargo_test("exercises", &name);
            let marker = self.has_marker(&name);
            match (outcome, marker) {
                (Outcome::Pass, false) => {
                    println!("{GREEN}✓{RESET} {name}");
                    progress.insert(name.clone(), self.file_hash(&name));
                }
                (Outcome::Pass, true) => {
                    println!("{YELLOW}~{RESET} {name} (passes, marker still present)");
                    all_ok = false;
                }
                (Outcome::Fail(_), _) => {
                    println!("{RED}✗{RESET} {name}");
                    all_ok = false;
                }
            }
        }
        self.save_progress(&progress);
        all_ok
    }

    /// For course authors: every solution must pass, and every shipped
    /// exercise must still carry the marker and must NOT pass as-is.
    fn check_solutions(&self, prefix: Option<&str>) -> bool {
        let mut ok = true;
        for name in self.exercises() {
            if let Some(p) = prefix {
                if !name.starts_with(p) {
                    continue;
                }
            }
            let mut problems = Vec::new();
            if !self.solution_path(&name).exists() {
                problems.push("missing solution file".to_string());
            } else if let Outcome::Fail(out) = self.cargo_test("solutions", &name) {
                problems.push(format!("solution fails:\n{out}"));
            } else if let Outcome::Fail(out) = self.cargo_clippy_solution(&name) {
                problems.push(format!("solution is not clippy-clean:\n{out}"));
            }
            if fs::read_to_string(self.solution_path(&name))
                .map(|s| s.lines().any(|l| l.trim() == MARKER))
                .unwrap_or(false)
            {
                problems.push("solution still contains the marker".to_string());
            }
            if !self.has_marker(&name) {
                problems.push("exercise is missing the marker".to_string());
            }
            if self.cargo_test("exercises", &name) == Outcome::Pass {
                problems.push("exercise already passes as shipped (nothing to do!)".to_string());
            }
            if self.hint(&name).is_none() {
                problems.push(format!("no `## {name}` section in its chapter's hints.md"));
            }
            if problems.is_empty() {
                println!("{GREEN}✓{RESET} {name}");
            } else {
                ok = false;
                println!("{RED}✗ {name}{RESET}");
                for p in problems {
                    println!("    {p}");
                }
            }
        }
        if !self.manifests_in_sync() {
            ok = false;
            println!("{RED}✗ [[test]] lists are out of date: run `cargo rl sync-manifests`{RESET}");
        }
        for path in self.files_in("solutions") {
            if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) {
                if !self.exercise_path(&stem).exists() {
                    ok = false;
                    println!("{RED}✗ solution {stem} has no matching exercise{RESET}");
                }
            }
        }
        ok
    }
}

fn main() -> ExitCode {
    let course = Course::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);

    let ok = match arg(0) {
        None | Some("next") => course.next(),
        Some("watch") => {
            course.watch();
            true
        }
        Some("list") => {
            course.list();
            true
        }
        Some("run") => match arg(1).and_then(|p| course.resolve(p)) {
            Some(name) => {
                let mut progress = course.load_progress();
                let done = course.run_one(&name, &mut progress);
                course.save_progress(&progress);
                done
            }
            None => false,
        },
        Some("hint") => {
            let name = match arg(1) {
                Some(p) => course.resolve(p),
                None => course.first_unfinished(&course.load_progress()),
            };
            match name {
                Some(name) => {
                    match course.hint(&name) {
                        Some(h) => println!("{BOLD}Hint for {name}{RESET}\n\n{h}"),
                        None => println!("No hint written for {name}."),
                    }
                    true
                }
                None => false,
            }
        }
        Some("solution") => {
            let name = match arg(1) {
                Some(p) => course.resolve(p),
                None => course.first_unfinished(&course.load_progress()),
            };
            match name {
                Some(name) => {
                    println!(
                        "Solution: {}\n{DIM}Try hard first — reading it too early skips the learning.{RESET}",
                        course.rel(&course.solution_path(&name))
                    );
                    true
                }
                None => false,
            }
        }
        Some("verify") => course.verify(),
        Some("reset-progress") => {
            let _ = fs::remove_file(course.progress_file());
            println!("Progress cache cleared.");
            true
        }
        Some("check-solutions") => course.check_solutions(arg(1)),
        Some("sync-manifests") => course.sync_manifests(),
        Some(other) => {
            eprintln!("Unknown command `{other}`. See the top of learn-rust/tooling/rl/src/main.rs.");
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
