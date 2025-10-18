use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use btreemap_decl as decl;
use btreemap_proc as procver;

#[test]
fn decl_empty() {
    let m: BTreeMap::<u8, u8> = decl::btreemap!();
    assert!(m.is_empty());
}

#[test]
fn proc_empty() {
    let m: BTreeMap::<&str, i32> = procver::btreemap!();
    assert!(m.is_empty());
}

#[test]
fn decl_basic() {
    let m = decl::btreemap! {
        "a" => 1,
        "b" => 2,
        "c" => 3,
    };
    assert_eq!(m.len(), 3);
    assert_eq!(m.first_key_value().map(|(k, _)| *k), Some("a"));
    assert_eq!(m.last_key_value().map(|(k, _)| *k), Some("c"));
}

#[test]
fn proc_basic() {
    let m = procver::btreemap! {
        3 => "c",
        1 => "a",
        2 => "b",
    };
    let keys: Vec<_> = m.keys().copied().collect();
    assert_eq!(keys, vec![1, 2, 3]);
}

#[test]
fn decl_overwrite_last_wins() {
    let m = decl::btreemap! {
        "x" => 1,
        "x" => 2,
    };
    assert_eq!(m.get("x"), Some(&2));
}

#[test]
fn proc_overwrite_last_wins() {
    let m = procver::btreemap! {
        "x" => 1,
        "x" => 2,
        "y" => 10,
    };
    assert_eq!(m.get("x"), Some(&2));
    assert_eq!(m.get("y"), Some(&10));
}

#[test]
fn decl_exprs_evaluated_once() {
    let counter = Rc::new(Cell::new(0));
    let bump = || {
        let n = counter.get();
        counter.set(n + 1);
        n
    };
    let m = decl::btreemap! {
        bump() => bump(),
        bump() => bump(),
    };
    assert_eq!(counter.get(), 4);
    assert_eq!(m.len(), 2);
}

#[test]
fn proc_exprs_evaluated_once() {
    let counter = Rc::new(Cell::new(0));
    let bump = || {
        let n = counter.get();
        counter.set(n + 1);
        n
    };
    let m = procver::btreemap! {
        bump() => bump(),
        bump() => bump(),
        bump() => bump(),
    };
    assert_eq!(counter.get(), 6);
    assert_eq!(m.len(), 3);
}

#[test]
fn interop_types() {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct K(&'static str);
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct V(u8);

    let a = decl::btreemap! {
        K("a") => V(1),
        K("b") => V(2),
    };
    let b = procver::btreemap! {
        K("c") => V(3),
        K("d") => V(4),
    };

    assert_eq!(a.get(&K("a")), Some(&V(1)));
    assert_eq!(b.get(&K("d")), Some(&V(4)));
}
