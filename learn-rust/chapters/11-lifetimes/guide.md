# 11 — Lifetimes

**Why this matters for the databroker.** The databroker's whole access-
control design is built out of *borrowing views*: you never touch the
`Database` directly — you get a `DatabaseReadAccess<'a, 'b>` that borrows the
database (`'a`) and your permissions (`'b`), and every method on it checks
those permissions. The lifetime parameters are what make this safe and free:
a view can't outlive the lock guard it came from, can't be used after the
permissions are gone, and a write view can't coexist with any read view.
When you see `impl<'a> Iterator for EntryReadIterator<'a, '_>` you need to
read it as fluently as `Vec<i32>`. That's what this chapter is for.

> Lifetimes feel hard because they're usually explained as a *feature*.
> They're not — they're a **description**. You never make something live
> longer by writing `'a`. You only *tell the compiler which borrow a
> reference came from*, so it can check that you never use it after the
> owner is gone.

---

## What a lifetime is (and isn't)

Every reference `&T` points at a value owned by someone else. The reference
is only valid while that owner is alive and not being mutated. The span of
code where a borrow must stay valid is its **lifetime**.

```text
fn main() {
    let db = Database::new();          // ─┐ db lives here
    let r = &db;                       //  │ ─┐ r borrows db
    println!("{}", r.len());           //  │  │ last use of r
                                       //  │ ─┘
    drop(db);                          // ─┘ ok: r is no longer used
}
```

Most of the time the compiler works all this out itself and you write no
lifetimes at all. You write a lifetime **name** (`'a`, `'db`, …) only where a
*signature* has to say how the borrows of inputs and outputs are related —
because the compiler checks each function using only its signature, never
the bodies of the functions it calls.

A lifetime name is a **generic parameter**, just like `T`:

```rust
fn first<'p>(paths: &'p [String]) -> &'p str
//      ^^^^ declare         ^^^ use          ^^^ use
```

Read `'p` as: "the returned `&str` points into the same thing `paths` points
into, so it's valid exactly as long as that is."

Two lifetimes are special:
- `'static` — valid for the whole program (string literals, `static`s,
  owned data with no borrows). `T: 'static` means "T contains no short-lived
  borrows" — *not* "T lives forever". A `String` is `'static`; a `&'a str`
  generally isn't.
- `'_` — "some lifetime; I don't need to name it". Used in `impl` headers and
  types where naming would add noise.

## Elision: the rules you already rely on

You've written `fn name(&self) -> &str` many times without lifetimes. That
works because of three **elision rules**:

1. Each reference parameter gets its own fresh lifetime.
2. If there is **exactly one** input lifetime, it's assigned to all outputs.
3. If there's a `&self` or `&mut self`, **its** lifetime is assigned to all
   outputs.

```rust
fn first_segment(path: &str) -> &str        // rule 2: output borrows `path`
fn path(&self) -> &str                      // rule 3: output borrows `self`
fn longer(a: &str, b: &str) -> &str         // two inputs, no self → E0106
```

For `longer` the compiler won't guess: the answer could be "from `a`", "from
`b`", or "from either". You say which:

```rust
fn longer<'p>(a: &'p str, b: &'p str) -> &'p str
```

Using one name for both inputs means: "the output is valid only while
**both** `a` and `b` are" — the compiler picks the overlap.

## Choosing which input the output borrows from

Giving every input the same lifetime compiles, but it may promise *more
dependence than is real*. A lookup is the classic case:

```rust
// Too strict: the result appears to depend on the key.
fn lookup<'a>(db: &'a HashMap<String, Entry>, key: &'a str) -> Option<&'a Entry>

// Right: the result points into the map; the key is only used during the call.
fn lookup<'db>(db: &'db HashMap<String, Entry>, key: &str) -> Option<&'db Entry>
```

With the strict version, this fails even though it's perfectly safe:

```rust
let entry = {
    let key = format!("Vehicle.{}", "Speed");   // temporary key
    lookup(&db, &key)
};                                             // key dropped here …
use_it(entry);                                 // … E0597 "key does not live long enough"
```

The databroker gets this right: `get_entry_by_path(&self, path: impl AsRef<str>)
-> Result<&Entry, ReadError>` (`databroker/src/broker.rs:1148`) — the path is
taken *by value* (any string-like thing), so it can't possibly be tied to the
result.

**When you can't return a reference at all.** If the value is *created*
inside the function, nobody else owns it; when the function returns it's
dropped. No lifetime annotation can fix that (E0515). Return the owned value:

```rust
fn normalized(path: &str) -> &str { let s = path.replace('/', "."); &s } // E0515
fn normalized(path: &str) -> String { path.replace('/', ".") }            // ✓
```

Rule of thumb: *annotations describe; they never extend*.

## Structs that borrow

A struct field that is a reference needs a lifetime parameter on the struct:

```rust
pub struct DatabaseReadAccess<'a, 'b> {     // databroker/src/broker.rs:1062
    db: &'a Database,
    permissions: &'b Permissions,
}
```

Read it as: "a `DatabaseReadAccess` borrows a `Database` for `'a` and some
`Permissions` for `'b`; it can't outlive either". Picture it:

```text
   owner of the DB (the lock guard)          owner of the Permissions (the gRPC request)
   ┌─────────────────────────┐               ┌──────────────────────┐
   │ Database                │               │ Permissions          │
   └─────────────────────────┘               └──────────────────────┘
              ▲  'a                                   ▲  'b
              │                                       │
           ┌──┴───────────────────────────────────────┴──┐
           │ DatabaseReadAccess { db, permissions }       │  ← 16 bytes, owns nothing
           └──────────────────────────────────────────────┘
```

Why two lifetimes and not one? Because the two things come from unrelated
places and live for different spans. With one `'a` the compiler would have to
shrink both to the shorter span; with two it can track them independently —
which matters as soon as a method returns something borrowed from only one of
them.

**Naming.** The databroker uses `'a`/`'b`. Nothing wrong with that — but
meaningful names make signatures self-documenting. This course uses `'db`
and `'perm` in exercises; `DatabaseReadAccess<'db, 'perm>` tells you at a
glance which borrow is which.

**Returning borrows from a view.** A view is short-lived; the entries it hands
out live in the database. Compare:

```rust
impl<'db, 'perm> ReadView<'db, 'perm> {
    fn get(&self, path: &str) -> Result<&Entry, ReadError>      // elided: tied to &self (the view)
    fn get(&self, path: &str) -> Result<&'db Entry, ReadError>  // tied to the database
}
```

The second is strictly more useful: you can drop the view (and even the
permissions) and keep using the entry — as long as the database is alive.
That's `c11_e03`.

## Enums that borrow, and '_

Enums work the same way:

```rust
pub enum EntryReadAccess<'a> {                 // databroker/src/broker.rs:1072
    Entry(&'a Entry),
    Err(&'a Metadata, ReadError),
}
```

Both variants borrow from the same database, so one lifetime suffices. When
iterating all entries, an unreadable entry is not skipped: the caller gets
`Err(&metadata, PermissionDenied)` — it may know the signal exists (metadata
is public) but not see the value.

Look at the two `impl` blocks for it:

```rust
impl EntryReadAccess<'_> {                     // :1077 — methods don't care which lifetime
    pub fn datapoint(&self) -> Result<&Datapoint, ReadError> { … }
    pub fn metadata(&self) -> &Metadata { … }
}

impl<'a> EntryReadAccess<'a> {                 // :1102 — `new` must *connect* 'a to its input
    fn new(entry: &'a Entry, permissions: &Permissions) -> Self { … }
}
```

- `impl<'a> Foo<'a>` — declares `'a` so signatures in the block can use it.
- `impl Foo<'_>` — "for any lifetime; I won't mention it". Equivalent to
  `impl<'a> Foo<'a>` when `'a` isn't used.
- `Self` inside `impl<'a> EntryReadAccess<'a>` means `EntryReadAccess<'a>`, so
  `new` says: "give me an `&'a Entry`, I'll give you an `EntryReadAccess<'a>`".
  Note `permissions: &Permissions` has its own anonymous lifetime — it's only
  *used* during `new`, not stored.

And the iterator (chapter 10):

```rust
pub struct EntryReadIterator<'a, 'b> {         // :1114
    inner: std::collections::hash_map::Values<'a, i32, Entry>,
    permissions: &'b Permissions,
}
impl<'a> Iterator for EntryReadIterator<'a, '_> {   // :1119
    type Item = EntryReadAccess<'a>;
```

The items carry the **database** lifetime `'a`, not the permissions'. You
could collect them into a `Vec` and drop the iterator.

## The databroker's authorized-access pattern

Here's the full chain, from the gRPC request down to one entry. Keep this
diagram next to `broker.rs` while reading:

```text
DataBroker (cheap Clone: Arcs inside)
  │  .authorized_access(&permissions)                     broker.rs:2352
  ▼
AuthorizedAccess<'a, 'b> { broker: &'a DataBroker, permissions: &'b Permissions }   :1373
  │  async fn get_datapoint(&self, id) {
  │      self.broker.database.read().await                ← lock guard (temporary)
  │          .authorized_read_access(self.permissions)    ← DatabaseReadAccess<'guard, 'b>  :1351
  │          .get_entry_by_id(id)                          ← Result<&'guard Entry, _>
  │          .map(|entry| entry.datapoint.clone())         ← CLONE: guard dies at `;`
  │  }
  ▼
Datapoint (owned) returned to the gRPC handler
```

Two things to notice:

1. `Database::authorized_read_access<'a, 'b>(&'a self, permissions: &'b Permissions)
   -> DatabaseReadAccess<'a, 'b>` (`:1351`) — the view borrows *the database
   inside the lock guard*. The guard is a temporary that dies at the end of
   the statement, so the view (and any `&Entry` from it) dies too. **That's
   why almost every `AuthorizedAccess` method ends in `.clone()` or
   `.cloned()`**: to get data *out* of the lock, you must own it. This is
   the borrow checker enforcing "don't use data after releasing the lock" —
   a bug class that causes real crashes in C/C++ and data races in Java.
2. `authorized_write_access(&'a mut self, …)` (`:1362`) takes `&mut`. Because a
   `&mut` borrow is exclusive, **while a `DatabaseWriteAccess` exists, no
   `DatabaseReadAccess` can**. The type system encodes the reader/writer rule
   even before the `RwLock` does.

`with_read_lock` (`:1411`) is the escape hatch for doing several reads under
one lock: instead of returning the view (impossible — it borrows a temporary
guard), it *lends* the view to your closure:

```rust
pub async fn with_read_lock<T>(&self, f: impl FnOnce(&DatabaseReadAccess) -> T) -> T
```

Your closure can use the view for as long as it runs; it returns an owned
`T`.

## Mental model

| Concept | Analogy |
|---------|---------|
| Reference `&T` | A library loan slip. Valid only while the library (owner) is open. |
| Lifetime `'a` | The date range printed on the slip. You don't choose it; you *write down* which library issued it. |
| `&mut T` | The only key to the reading room — nobody else gets a slip while you have it. |
| `'static` | A slip for a book you own — never expires (not "the book lives forever"). |
| Struct with `'a` | A folder of loan slips: the folder is only valid while every slip in it is. |
| Elision | The librarian fills in the obvious date for you when there's only one possible library. |

**What the compiler guarantees:** no dangling references (use-after-free is
impossible), no reference escapes its owner (E0515, E0597), and no data is
read while being mutated through another path (E0502). All checked at
compile time — lifetimes cost nothing at runtime.

## Common misconceptions

| Wrong model | Correct model | Error |
|-------------|---------------|-------|
| "`'a` makes the value live longer" | Annotations only *describe* relationships; they never change when anything is dropped | E0597 |
| "Just put `'a` on every reference" | Tie outputs only to the inputs they really borrow from | E0597 on a temporary argument |
| "I can return a reference to something I just built" | The builder owns it and drops it; return the owned value | E0515 |
| "`T: 'static` means T lives forever" | It means T holds no non-`'static` borrows; owned types qualify | E0310 |
| "Methods returning refs always borrow from `self`" | Only by elision default; a view can return `&'db T` that outlives the view | E0597 |
| "Two lifetimes is over-engineering" | Separate owners with separate lifespans need separate names | — |
| "Lifetimes are like GC" | They're compile-time scope checks; there's no runtime tracking | — |

## Smells and idioms

| Smell | Idiomatic |
|-------|-----------|
| `.clone()` to escape a lifetime error without understanding it | First ask *who should own this?* — maybe the signature ties the output to the wrong input |
| `fn f<'a>(x: &'a str, y: &'a str)` when only `x` is returned | Name only the lifetime that matters |
| `&'static str` param to "fix" errors | Accept `&str`/`impl AsRef<str>`; `'static` restricts callers |
| Storing `&T` in a long-lived struct (a cache, a task) | Own the data (`String`, `Arc<T>`) — borrowing structs are for short-lived views |
| `Box::leak` to get `'static` | Almost never; use `Arc` or restructure |
| `'a`, `'b`, `'c` in a long signature | Descriptive names: `'db`, `'perm`, `'req` |

The databroker's views are the textbook-correct use of borrowing structs:
short-lived, created per operation, zero-cost. Where it needs long-lived
shared data (a spawned task, a provider in a map) it uses `Arc` instead —
chapter 12.

## In the databroker

- `databroker/src/broker.rs:1062`–`1070` — `DatabaseReadAccess<'a, 'b>` and
  `DatabaseWriteAccess<'a, 'b>` (note `&'a mut Database` in the write one).
- `databroker/src/broker.rs:1072`–`1112` — `EntryReadAccess<'a>` plus both
  impl blocks (`impl EntryReadAccess<'_>` and `impl<'a> EntryReadAccess<'a>`).
- `databroker/src/broker.rs:1114`–`1133` — `EntryReadIterator<'a, 'b>` and
  `impl<'a> Iterator for EntryReadIterator<'a, '_>`.
- `databroker/src/broker.rs:1148` — `get_entry_by_path(&self, path: impl AsRef<str>)`:
  the key isn't tied to the result.
- `databroker/src/broker.rs:1167` — `iter_entries(&self) -> EntryReadIterator<'_, '_>`:
  both lifetimes elided to `self`'s.
- `databroker/src/broker.rs:1351`, `:1362` — `authorized_read_access<'a, 'b>`
  and `authorized_write_access<'a, 'b>(&'a mut self, …)`.
- `databroker/src/broker.rs:1373` — `AuthorizedAccess<'a, 'b>`: the same
  pattern one level up, borrowing the whole `DataBroker`.
- `databroker/src/broker.rs:1411`–`1490` — `with_read_lock` and the
  `get_*` methods: lock, view, look up, **clone out**.
- `databroker/src/broker.rs:2352` — `DataBroker::authorized_access<'a, 'b>`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:1308` — `get_signal(…, broker: &AuthorizedAccess<'_, '_>)`:
  the `'_` shorthand in a parameter type.

## Exercises

| Exercise | Goal |
|----------|------|
| `c11_e01_elision` | See the three elision rules; fix E0106 with explicit lifetimes |
| `c11_e02_precise_lifetimes` | Tie outputs to the right input; fix E0515 by returning owned data |
| `c11_e03_struct_holding_refs` | Build `ReadView<'db, 'perm>`; return `&'db Entry` that outlives the view |
| `c11_e04_enum_holding_refs` | Implement `EntryReadAccess<'a>` with `impl<'a>` and `impl …<'_>` |
| `c11_e05_scoped_authorized_view` | Full read/write authorized views over a mini database |
| `c11_e06_build_vss_path_parser` | build: a lifetime-carrying VSS path parser (`common_prefix`, `is_descendant`) |
| `c11_e07_review_path_tracking` | review: find and fix 3 lifetime-correct-but-wrong bugs |

## Further reading

- The Rust Book, 10.3 "Validating References with Lifetimes" —
  https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
- The Rust Reference, "Lifetime elision" —
  https://doc.rust-lang.org/reference/lifetime-elision.html
- "Common Rust Lifetime Misconceptions" (pretzelhammer) —
  https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md
- Edition 2024 note: return-position `impl Trait` now captures all in-scope
  lifetimes by default, so the `+ '_` you write in `c11_e05` becomes implicit.
  The databroker is edition 2021, where you still need it.

## Check yourself

1. Why does `fn longer(a: &str, b: &str) -> &str` need an annotation but
   `fn name(&self, other: &str) -> &str` doesn't? Which input does the
   latter's output borrow from?
2. Why does `DatabaseReadAccess` have *two* lifetime parameters?
3. Why do most `AuthorizedAccess` getters end in `.clone()`?
4. What prevents holding a `DatabaseReadAccess` and a `DatabaseWriteAccess`
   at the same time?
5. What does `impl EntryReadAccess<'_>` mean, and when do you need
   `impl<'a> EntryReadAccess<'a>` instead?
6. Is `String: 'static`? Is `&'a str: 'static`?

<details><summary>Answers</summary>

1. Two reference inputs and no `self` → no elision rule applies. With
   `&self`, rule 3 ties the output to `self` (so returning `other` would
   actually be an error unless you annotate).
2. The database and the permissions have different owners (lock guard vs.
   request) and different lifespans; separate parameters let methods return
   borrows of one without being limited by the other.
3. The database reference comes from a lock guard that's dropped at the end
   of the statement. To return data past that point you must own it.
4. `authorized_write_access` needs `&mut Database`, and a `&mut` borrow is
   exclusive: no `&Database` (needed for a read view) may coexist with it.
5. `'_` = "any lifetime, unnamed" — fine when no signature in the block needs
   to refer to it. When a method must connect the struct's lifetime to an
   argument or return type (like `new(entry: &'a Entry) -> Self`), declare it.
6. Yes — a `String` owns its data, containing no borrows. `&'a str` is
   `'static` only when `'a` is `'static` (e.g. a string literal).

</details>

## Where this goes next

**Chapter 16 (Advanced lifetimes)** continues from here: `T: 'static` vs `&'static`, lifetime bounds on trait objects, higher-ranked bounds (`for<'a>`), variance, reborrowing, and why self-referential structs lead to `Pin` (chapter 24).
