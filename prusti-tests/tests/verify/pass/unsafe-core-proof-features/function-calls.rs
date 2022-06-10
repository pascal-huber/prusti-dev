// compile-flags: -Punsafe_core_proof=true
use prusti_contracts::*;
fn main() {}

fn function_call_one_arg() {
    let mut a = 1;
    let x = f1(&mut a);
}
fn f1(x: &mut i32){}

fn function_call_two_arg() {
    let mut a = 1;
    let mut b = 1;
    let x = f2(&mut a, &mut b);
    a = 4;
}
fn f2<'b:'a, 'a>(x: &'a mut i32, y: &'b mut i32){}

fn function_call_two_arg_with_modifiction() {
    let mut a = 1;
    let mut b = 1;
    let x = f3(&mut a, &mut b);
    a = 3;
    b = 3;
}
fn f3<'a, 'b>(x: &'a mut i32, y: &'b mut i32){
    *x = 2;
    *y = 2;
}

fn function_call_return_value() {
    let mut a = 1;
    let x = f4(&mut a);
}
fn f4(x: &mut i32) -> i32{
    *x
}

// TODO: fix function reference return value
// fn function_call_ref_return_value() {
//     let mut a = 1;
//     let x = f4(&mut a);
// }
// fn f5(x: &mut i32) -> &mut i32{
//     x
// }
