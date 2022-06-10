// compile-flags: -Punsafe_core_proof=true

use prusti_contracts::*;

pub fn test1() {
    let mut a = 4;
    let x = &mut a;
}

fn main() {}
