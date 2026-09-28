# Chapter 11 hints

## c11_e01_elision
`rustc --explain E0106`. The compiler asks "this function's return type
contains a borrowed value, but the signature does not say whether it is
borrowed from `a` or `b`".

Declare a lifetime parameter after the function name and put it on every
reference that the result might come from, and on the result:
`fn longer_path<'p>(a: &'p str, b: &'p str) -> &'p str`.
Do the same for `pick_path` — the result can come from `paths` or `fallback`.

## c11_e02_precise_lifetimes
`lookup`: if you write `<'a>(db: &'a .., path: &'a str) -> Option<&'a Entry>`
it compiles *here* but the temporary-key test fails with "`key` does not
live long enough": you promised the result depends on `path`. Put the named
lifetime only on `db` and the output; leave `path` plain.

`normalized`: `rustc --explain E0515`. `dotted` is dropped when the function
returns, so no reference to it can survive. Return the `String` itself (the
tests compare it with `==` against `&str`, which works for `String`).

## c11_e03_struct_holding_refs
Step 1: `struct ReadView<'db, 'perm> { db: &'db Database, permissions: &'perm Permissions }`.
Step 2: the impl header must mention the parameters: `impl<'db, 'perm> ReadView<'db, 'perm>`
(or `impl<'db> ReadView<'db, '_>` since `'perm` isn't used in signatures).

Step 3: now you'll see E0597 in the test `entry_outlives_view_and_permissions`.
`fn get(&self, ..) -> Result<&Entry, _>` — elision rule 3 ties the output to
`&self`, i.e. to the *view*. Say what you mean: `-> Result<&'db Entry, ReadError>`.
The body compiles unchanged because `self.db` is a `&'db Database`.

## c11_e04_enum_holding_refs
`new`: `match permissions.can_read(&entry.metadata.path)`; `Ok(())` →
`Self::Entry(entry)`, `Err(err)` → `Self::Err(&entry.metadata, err)`.

`datapoint`: match on `self`. For `Self::Entry(entry)` return
`Ok(&entry.datapoint)`. For `Self::Err(_, err)`, `err` is a `&ReadError`; the
return needs an owned `ReadError`, so `err.clone()`.

`metadata`: both arms return a `&Metadata`: `&entry.metadata` or the stored
reference itself.

## c11_e05_scoped_authorized_view
The two constructors are one struct literal each (field init shorthand:
`ReadAccess { db: self, permissions }`).

`entry_by_path`: `path_to_id.get(path)` → `Option<&i32>`; `.ok_or(AccessError::NotFound)?`
converts to a Result and propagates. Then `entries.get(id)`, then the
permission check. The result must be `&'db Entry`: `self.db` is
`&'db Database` and indexing through it yields `'db` references. If the
compiler complains, bind `let db: &'db Database = self.db;` first.

`readable_paths`: `self.db.entries.values().filter(..).map(|e| e.path.as_str())`.
Remove the `todo!()` and the placeholder `std::iter::empty()` — your
iterator chain is the return value.

`update_by_path`: copy the id out (`*...get(path).ok_or(..)?`) so you don't
hold a borrow of `path_to_id` while taking `entries.get_mut(&id)`.
`std::mem::replace(&mut entry.value, value)` swaps in the new value and
returns the old one.

## c11_e06_build_vss_path_parser
`parse`: `path.rsplit('.').next().unwrap_or(path)` gives the last segment
whether or not there's a dot. `common_prefix`: zip `a.split('.')` with
`b.split('.')`, walk them together accumulating a byte length, `break` at
the first mismatched pair, then slice `&a[..len]` — notice the result only
ever borrows from `a`. `is_descendant`: `child == parent` or
(`child.starts_with(parent)` and the byte right after `parent` is `.`);
that second check is what rules out `"Vehicle.CabinLight"` as a descendant
of `"Vehicle.Cabin"`.

## c11_e07_review_path_tracking
Three independent bugs — the borrow checker has no opinion on any of them:
- `LatestPath::update` pushes onto `seen` but never touches `self.current`.
  Add the missing assignment.
- `first_segment` just returns its argument. Split on `.` and take the
  first piece.
- `longer_of`'s `if` condition is backwards, so it always returns the
  shorter string (and gives ties to the wrong side). Work out by hand what
  `a.len() < b.len()` evaluates to for `("abc", "de")` and you'll see it.
