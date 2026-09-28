# Chapter 12 hints

## c12_e01_box_recursive
`rustc --explain E0072`. Wrap the recursive fields: `And(Box<Filter>, Box<Filter>)`
and `Not(Box<Filter>)`. The helper constructors then need `Box::new(a)`.

`matches` and `depth` are `match self { ... }` with one arm per variant.
Calling a method on a `&Box<Filter>` auto-derefs, so `a.matches(path)` just
works. For depth: `1 + a.depth().max(b.depth())`.

## c12_e02_drop_raii
`subscribe`: `self.active.set(self.active.get() + 1)`, push
`format!("subscribe {name}")` via `self.log.borrow_mut().push(..)`, return
`SubscriptionHandle { name: name.to_string(), broker: self }`.

`impl Drop for SubscriptionHandle<'_> { fn drop(&mut self) { ... } }`:
decrement `self.broker.active` and push `"unsubscribe <name>"`.

`short_lived_then_check`: `drop(handle); broker.active()`. (`drop` is just
a function that takes ownership and lets the value go out of scope.)

## c12_e03_rc_refcell
`add`: `self.subscribers.push(Rc::clone(subscriber))`.
`notify`: loop over `&self.subscribers`, `s.borrow_mut().received.push(value)`.
`peek_len`: `try_borrow()` returns `Result<Ref<_>, BorrowError>`;
`.ok().map(|s| s.received.len())`.

## c12_e04_arc_shared
`fan_out`: `(0..n).map(|_| Arc::clone(meta)).collect()`.
`available_providers`: `providers.iter().filter(|p| p.is_available()).cloned().collect()`
— `.cloned()` on an iterator of `&Arc<_>` clones the Arcs (cheap).

## c12_e05_mutex_rwlock
- `set`: `self.values.write().expect("lock poisoned").insert(path.to_string(), value);`
  then `*self.writes.lock().expect("lock poisoned") += 1;`. Each guard is a
  temporary and is dropped at the `;`.
- `value`: `self.values.read().expect(..).get(path).copied()`.
- `snapshot`: `.read().expect(..).clone()` — clones the map, not the lock.
- `with_read`: `let guard = self.values.read().expect(..); f(&guard)`.
  `&guard` derefs to `&HashMap<..>`.

## c12_e06_atomics
`rustc --explain E0277`. Swap `Cell<i32>` → `AtomicI32`, `Cell<bool>` →
`AtomicBool` (from `std::sync::atomic`), construct with `AtomicI32::new(first)`.

`allocate`: a get-then-set is two steps — two threads can both read 5. Use
the single-step `self.next.fetch_add(1, Ordering::Relaxed)`, which returns
the previous value.

The flag: `store(true, Ordering::Release)` / `load(Ordering::Acquire)`.
(`SeqCst` everywhere also passes; the point is to know why weaker is enough.)

## c12_e07_oncelock_lazylock
`rustc --explain E0015`.
`static ALLOW_ALL: LazyLock<Permissions> = LazyLock::new(Permissions::all);`
(`use std::sync::LazyLock`). Accessing `ALLOW_ALL.readable` derefs through
the LazyLock and initialises it on first use.

`config`: declare `static CONFIG: OnceLock<Config> = OnceLock::new();` inside
the function and return `CONFIG.get_or_init(load_config)`.

## c12_e08_weak_parent_links
Parents own children (`Rc`), children only *point back* (`Weak`).

`add_child`: build the child with `parent: RefCell::new(Rc::downgrade(parent))`,
push `Rc::clone(&child)` into `parent.children.borrow_mut()`, return `child`.

`full_path`: start with the node's own name, then loop
`current = node.parent.borrow().upgrade()`: while it's `Some(p)`, push
`p.name` and move up. Reverse and `join(".")`. When the parent has been
dropped, `upgrade()` gives `None` and the walk simply stops.

## c12_e09_build_subscription_registry
`subscribe`: `Rc::new(RefCell::new(Vec::new()))`, then
`self.subs.entry(path.to_string()).or_default().push(Rc::downgrade(&log))`,
then return `log`. `publish`: `Vec::retain` on the `Vec<Weak<_>>` — inside
the closure, `match weak.upgrade() { Some(log) => { push, count it, keep
(true) }, None => drop it (false) }`. That single `retain` call delivers
and prunes in one pass. `live_count` is the same upgrade-and-filter idea,
just counting instead of mutating.

## c12_e10_review_shared_state_bugs
Three unrelated bugs in three unrelated pieces of code:
- `record_and_total`: the `let mut readings = sensor.readings.borrow_mut();`
  binding keeps that guard alive for the rest of the function, so `total`'s
  `.borrow()` panics. Don't bind it — `sensor.readings.borrow_mut().push(value);`
  as its own statement drops the guard immediately after.
- `Node`: read the field doc/rule "parents own children, children only
  point back" from `c12_e08`. `parent: RefCell<Option<Rc<Node>>>` breaks
  that rule; make it `Weak<Node>` and use `Rc::downgrade` in `attach_child`.
- `Cache::mirror`: `Rc::new(RefCell::new(value))` builds a fresh, unrelated
  cell. To actually share, clone the `Rc` you already have: `Rc::clone(cell)`.
