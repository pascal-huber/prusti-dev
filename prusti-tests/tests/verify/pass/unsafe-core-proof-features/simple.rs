// compile-flags: -Punsafe_core_proof=true
use prusti_contracts::*;
fn main() {}

pub fn simple_assignment() {
    let mut a = 4;
    let x = &mut a;
    let y = x;
}

fn borrow_twice_and_assign() {
    let mut a: i32 = 4;
    let x = &mut a;
    let y = &mut *x;
    *y = 4;
}
