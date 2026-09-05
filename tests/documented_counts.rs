//! The counts the README states about the objects, asserted here so they
//! cannot quietly go stale.
//!
//! The README's claim is that the affordability question is closed: at a
//! 4,000-mode budget most of these objects fit **entirely**, several times
//! over. That sentence names numbers, and numbers in prose rot the moment an
//! object is added. This fails when they stop being true, which is the only
//! reason it exists.

use noob_resonator::dsp::object::{OBJECT_NAMES, Object, Shape};

/// The budget the README argues about.
const BUDGET: usize = 4_000;

/// The bottom of the range the claim is made at. Higher fundamentals have
/// fewer partials below 20 kHz, so this is the hard end.
const F0: f64 = 110.0;

fn partials(i: usize) -> usize {
    let s = Shape {
        object: Object::from_index(i),
        ..Shape::default()
    };
    s.available(20_000.0 / F0)
}

#[test]
fn the_readme_names_the_right_objects_as_fitting_the_budget() {
    // Named rather than counted: "eight of ten" stops meaning anything the
    // moment the tenth changes, and a count would still pass if two objects
    // swapped sides.
    let over: Vec<&str> = OBJECT_NAMES
        .iter()
        .enumerate()
        .filter(|(i, _)| partials(*i) > BUDGET)
        .map(|(_, n)| *n)
        .collect();
    assert_eq!(
        over,
        vec!["Membrane", "Membrane Round"],
        "the README says the two membranes are the objects that do not fit a \
         {BUDGET}-mode budget at {F0} Hz; the objects that do not fit are now \
         {over:?}. Fix the README, or the physics."
    );
}

#[test]
fn the_largest_object_that_fits_is_the_one_the_readme_quotes() {
    let (name, n) = OBJECT_NAMES
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, partials(i)))
        .filter(|(_, n)| *n <= BUDGET)
        .max_by_key(|(_, n)| *n)
        .expect("something has to fit");
    assert_eq!((name, n), ("Plate", 265), "the README quotes this pair");
    // "Several times over" is the README's phrase; hold it to at least ten.
    assert!(
        BUDGET / n >= 10,
        "{n} partials is not several times under {BUDGET}"
    );
}
