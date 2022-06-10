// compile-flags: -Punsafe_core_proof=true
use prusti_contracts::*;
fn main() {}

fn branch_borrow(c: bool){
    let mut a: i32 = 4;
    let mut b: i32 = 5;
    let x;
    if c {
        x = &mut a;
    } else {
        x = &mut b;
    }
}

fn branch_borrow_assign(c: bool) {
    let mut a: i32 = 4;
    let mut b: i32 = 5;
    let x;
    if c {
        x = &mut a;
    } else {
        x = &mut b;
    }
    *x = 6;
}
