# Chapter 29 hints

## c29_e01_string_methods
- `depth`: `path.split('.').count()` — but `"".split('.')` yields one empty
  item, so handle `""` first.
- `parent`: `path.rsplit_once('.')` returns `Option<(&str, &str)>`; `.map`
  it to the first half.
- `leaf`: same call, then `.map_or(path, |(_, leaf)| leaf)`.
- `join_path`: slices of `&str` have `.join(".")`.
- `to_slash_path`: `.replace('.', "/")` allocates a new `String`.
- `branch_of_recursive_glob`: `glob.strip_suffix("/**")` is exactly this.
Returned `&str`s borrow from the argument — lifetime elision ties them together.

## c29_e02_validate_vss_path
Segment check: `!segment.is_empty() && !segment.chars().any(|c| c == '.' || c == ':' || c == '*' || c.is_whitespace())`.
Path check: `path.split('.').all(is_valid_segment)` — a function name can be
passed where a closure is expected.
Regex: one segment is `[^.:*\s]+`; a path is a segment followed by zero or
more `\.segment` groups, anchored: `^[^.:*\s]+(?:\.[^.:*\s]+)*$`.
Initialise with `Regex::new(r"...").expect("static regex is valid")` inside
the `LazyLock::new(|| ...)` closure, then `VALID_PATH.is_match(path)` —
`LazyLock` derefs to the `Regex`, compiling it on first access.

## c29_e03_glob_to_matcher
`new`: pick the effective pattern (`""` or `"\"\""` → `"**"`), reject with
`is_valid_pattern`, then `let mut glob = pattern.replace('.', "/");` and
`glob.push_str("/**")` when `!pattern.contains('*')`.
`is_match`: convert the path with `replace('.', "/")`, try
`glob_match(&self.glob, &path)`; if that fails, try again with
`self.glob.strip_suffix("/**")` (use `is_some_and`).
`as_glob`: return `&self.glob` — `&String` coerces to `&str`.

## c29_e04_builder_pattern
`PathSet::add(&mut self, ...)`: match on the permission. `All` replaces
`*self` with `Everything`. For `Glob(g)`, match on `self`: `Nothing` becomes
`Globs(vec![g])`, `Everything` stays, `Globs(globs)` pushes.
Builder methods: `fn add_read_permission(mut self, p) -> Self { self.read.add(p); self }`.
`build`: loop over `[&self.read, &self.provide]`; for `PathSet::Globs(globs)`
use `globs.iter().find(|g| g.is_empty() || g.chars().any(char::is_whitespace))`
and return `Err(BuildError::InvalidGlob(bad.clone()))`. Then move the fields
into `Permissions` — `self` is owned, so no clones are needed there.

## c29_e05_permissions_can_read
`from_scope`: start with every matcher `Nothing`; for each item of
`scope.split_whitespace()`, `item.split_once(':')` gives `(action, path)` or
`None` (no path). Pick `&mut permissions.read` etc. with a `match` on the
action string, then `matcher.allow(path)`.
`is_expired`: `self.expires_at.is_some_and(|t| t < SystemTime::now())`.
`can_read`: expired → `Expired`; then `[&self.read, &self.actuate, &self.provide, &self.create].iter().any(|m| m.is_match(path))`.
The two write checks only look at one matcher each — a small private helper
`fn check(expired, matcher, path)` avoids repeating yourself.
