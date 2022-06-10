// compile-flags: -Punsafe_core_proof=true
use prusti_contracts::*;
fn main() {}

fn function_call_one_arg() {
    let mut a = 1;
    let x = f15_1(&mut a);
}
fn f15_1(x: &mut i32){}

fn function_call_two_arg() {
    let mut a = 1;
    let mut b = 1;
    let x = f15_2(&mut a, &mut b);
    a = 4;
}
fn f15_2<'b:'a, 'a>(x: &'a mut i32, y: &'b mut i32){}

fn function_call_two_arg_with_moditifiction() {
    let mut a = 1;
    let mut b = 1;
    let x = f15_3(&mut a, &mut b);
    a = 3;
    b = 3;
}
fn f15_3<'a, 'b>(x: &'a mut i32, y: &'b mut i32){
    *x = 2;
    *y = 2;
}

