// compile-flags: -Punsafe_core_proof=true
use prusti_contracts::*;
fn main() {}

// simple assigment
pub fn simple_assignment() {
    let mut a = 4; // _1 = const 4_i32
    let x = &mut a; // _2 = &'_#3r mut _1
    let y = x;
}

fn borrow_twice_and_assign() {
    let mut a: i32 = 4;
    let x = &mut a;
    let y = &mut *x;
    *y = 4;
}


