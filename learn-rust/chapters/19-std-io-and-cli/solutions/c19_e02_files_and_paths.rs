// c19_e02_files_and_paths.rs
//
// CONCEPT: `Path`/`PathBuf` (the `&str`/`String` of file names), `OsStr`,
//          `std::fs`, directory listing, and writing a file atomically.
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "Files and paths")
// DATABROKER: databroker/src/main.rs — `read_metadata_file` opens each `--vss`
//             file (line 159); `unlink_unix_domain_socket` takes
//             `impl AsRef<Path>` and checks metadata before removing (line 223);
//             line 592 creates the socket's parent directory.
// EXPECT:  tests fail: `todo!()` panics.
//
// File names are not `String`s: on Linux they are arbitrary bytes (except
// `/` and NUL), so `Path` wraps an `OsStr`, not a `str`. Converting to text
// is explicit and fallible (`to_str()` returns `Option`) or lossy
// (`display()`, `to_string_lossy()`). Build paths with `join`, never with
// `format!("{dir}/{name}")`.
//
// YOUR TASK
//   Implement the four `todo!()`s:
//     - `json_files(dir)`: the `.json` files directly inside `dir` (not in
//       sub-directories), sorted by path;
//     - `backup_path(path)`: `a/b.json` → `a/b.json.bak`, `a/b` → `a/b.bak`;
//     - `load_by_stem(dir)`: map file stem ("vss_4.0") → file contents for
//       every file from `json_files`;
//     - `write_atomically(path, contents)`: write to a temporary file in the
//       same directory, then `fs::rename` it over `path`, so readers never
//       see a half-written file. Create missing parent directories first.
//   Do not edit the tests.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn json_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        // WHY: `extension()` returns `Option<&OsStr>`; comparing with a `&str`
        // works because `OsStr: PartialEq<str>`. No UTF-8 conversion needed.
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }
    // WHY: `read_dir` order is whatever the file system returns; sort it.
    files.sort();
    Ok(files)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

fn load_by_stem(dir: &Path) -> io::Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for path in json_files(dir)? {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        map.insert(stem, fs::read_to_string(&path)?);
    }
    Ok(map)
}

fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, contents)?;
    // WHY: on the same file system, `rename` replaces the target in one step.
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A unique scratch directory that deletes itself when dropped (RAII).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "learn-rust-c19e02-{}-{name}-{n}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lists_only_json_files_sorted() {
        let tmp = TempDir::new("list");
        fs::write(tmp.0.join("vss_4.0.json"), "{}").unwrap();
        fs::write(tmp.0.join("notes.txt"), "x").unwrap();
        fs::write(tmp.0.join("vss_3.1.json"), "{}").unwrap();
        fs::create_dir(tmp.0.join("nested.json")).unwrap(); // a directory!
        fs::write(tmp.0.join("nested.json").join("inner.json"), "{}").unwrap();

        let files = json_files(&tmp.0).unwrap();
        assert_eq!(
            files,
            vec![tmp.0.join("vss_3.1.json"), tmp.0.join("vss_4.0.json")]
        );
    }

    #[test]
    fn missing_directory_is_an_error() {
        let err = json_files(Path::new("/definitely/not/here")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn backup_names() {
        assert_eq!(
            backup_path(Path::new("data/vss.json")),
            PathBuf::from("data/vss.json.bak")
        );
        assert_eq!(backup_path(Path::new("token")), PathBuf::from("token.bak"));
    }

    #[test]
    fn loads_contents_by_stem() {
        let tmp = TempDir::new("load");
        fs::write(tmp.0.join("vss_4.0.json"), r#"{"Vehicle":{}}"#).unwrap();
        fs::write(tmp.0.join("overlay.json"), "{}").unwrap();
        let map = load_by_stem(&tmp.0).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["vss_4.0"], r#"{"Vehicle":{}}"#);
        assert_eq!(map["overlay"], "{}");
    }

    #[test]
    fn atomic_write_creates_parents_and_replaces() {
        let tmp = TempDir::new("atomic");
        let target = tmp.0.join("run").join("kuksa").join("state.json");
        write_atomically(&target, "v1").unwrap();
        write_atomically(&target, "v2").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "v2");
        // No temporary file is left behind.
        let leftovers: Vec<_> = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![OsString::from("state.json")]);
    }
}
