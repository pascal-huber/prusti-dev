// compile-flags: -Punsafe_core_proof=true

use prusti_contracts::*;

fn main() {}

#[derive(Clone)]
struct Container<T>(T);

#[derive(Clone)]
struct I32Container<i32>(i32);

#[derive(Clone)]
struct Container2<T,U>{
    t: T,
    u: U,
}
