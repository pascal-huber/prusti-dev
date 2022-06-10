// compile-flags: -Punsafe_core_proof=true
use prusti_contracts::*;
fn main() {}

fn simple_struct() {
    let mut x = S1{ x: 3};
    let mut y = &mut x;
    y.x = 3;
    x.x = 2;
}
struct S1 {
    x: u32,
}
