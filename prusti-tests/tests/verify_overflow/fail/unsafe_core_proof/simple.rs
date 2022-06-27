// compile-flags: -Punsafe_core_proof=true

use prusti_contracts::*;
fn main() {}

pub fn mutable_borrow() {
    let mut a = 4;
    let x = &mut a;
}
pub fn mutable_borrow_assert_false() {
    let mut a = 4;
    let x = &mut a;
    assert!(false);      //~ ERROR: the asserted expression might not hold
}

pub fn shared_borrow() {
    let mut a = 4;
    let x = &a;
}
pub fn shared_borrow_assert_false() {
    let mut a = 4;
    let x = &a;
    assert!(false);      //~ ERROR: the asserted expression might not hold
}

pub fn shared_reborrow() {
    let mut a = 4;
    let x = &a;
    let y = &(*x);
}
pub fn shared_reborrow_assert_false() {
    let mut a = 4;
    let x = &a;
    let y = &(*x);
    assert!(false);      //~ ERROR: the asserted expression might not hold
}
