// compile-flags: -Punsafe_core_proof=true

use prusti_contracts::*;

#[requires(true)]
pub fn test1() {
    let mut a = 4;
    let x = &mut a;
    assert!(*x == 4);
}

fn main() {}
