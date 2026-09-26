// c12_e08_weak_parent_links.rs
//
// CONCEPT: `Weak<T>`: a non-owning pointer that breaks `Rc` reference cycles.
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Weak: breaking reference cycles")
// DATABROKER: the VSS tree (`vss.rs`) is flattened into a map keyed by full
//          path, which sidesteps parent pointers entirely. When you *do* need
//          a tree that can walk both down (children) and up (parent), this
//          is the standard shape.
// EXPECT:  tests fail: `todo!()` panics.
//
// A node owns its children (`Rc`), but a child must NOT own its parent: if it
// did, parent → child → parent would be a cycle, the strong counts would
// never reach zero, and the whole tree would leak. The child holds a
// `Weak<Node>` instead. `weak.upgrade()` returns `Some(Rc<Node>)` while the
// parent is alive and `None` once it has been dropped.
//
// YOUR TASK
//   Implement `add_child`, `full_path` and `child_names`.
//   Do not edit the tests.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Debug)]
struct Node {
    name: String,
    parent: RefCell<Weak<Node>>,
    children: RefCell<Vec<Rc<Node>>>,
}

impl Node {
    fn root(name: &str) -> Rc<Node> {
        Rc::new(Node {
            name: name.to_string(),
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(Vec::new()),
        })
    }
}

/// Creates a child called `name` under `parent`, links it both ways
/// (parent owns child, child points weakly to parent) and returns it.
fn add_child(parent: &Rc<Node>, name: &str) -> Rc<Node> {
    let child = Rc::new(Node {
        name: name.to_string(),
        // WHY: `Rc::downgrade` makes a Weak without bumping the strong count,
        // so the child doesn't keep its parent alive.
        parent: RefCell::new(Rc::downgrade(parent)),
        children: RefCell::new(Vec::new()),
    });
    parent.children.borrow_mut().push(Rc::clone(&child));
    child
}

/// The dotted VSS path from the root down to `node`, e.g.
/// `Vehicle.Cabin.Door`. Walks upwards through the weak parent links.
fn full_path(node: &Rc<Node>) -> String {
    let mut parts = vec![node.name.clone()];
    let mut current = node.parent.borrow().upgrade();
    while let Some(parent) = current {
        parts.push(parent.name.clone());
        current = parent.parent.borrow().upgrade();
    }
    parts.reverse();
    parts.join(".")
}

/// Names of `node`'s direct children, in insertion order.
fn child_names(node: &Rc<Node>) -> Vec<String> {
    node.children
        .borrow()
        .iter()
        .map(|child| child.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_walk_up_through_weak_parents() {
        let vehicle = Node::root("Vehicle");
        let cabin = add_child(&vehicle, "Cabin");
        let door = add_child(&cabin, "Door");
        add_child(&vehicle, "Speed");

        assert_eq!(full_path(&door), "Vehicle.Cabin.Door");
        assert_eq!(full_path(&vehicle), "Vehicle");
        assert_eq!(child_names(&vehicle), vec!["Cabin", "Speed"]);
    }

    #[test]
    fn children_do_not_keep_parents_alive() {
        let vehicle = Node::root("Vehicle");
        let cabin = add_child(&vehicle, "Cabin");

        // Only `vehicle` owns the root; the child's link is weak.
        assert_eq!(Rc::strong_count(&vehicle), 1);
        assert_eq!(Rc::weak_count(&vehicle), 1);
        // `cabin` is owned by us AND by the root's children list.
        assert_eq!(Rc::strong_count(&cabin), 2);

        drop(vehicle);

        // The root was freed, so the weak link now upgrades to None and the
        // child's path starts at itself.
        assert!(cabin.parent.borrow().upgrade().is_none());
        assert_eq!(full_path(&cabin), "Cabin");
        assert_eq!(Rc::strong_count(&cabin), 1);
    }
}
