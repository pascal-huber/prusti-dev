extern crate prusti_contracts;
use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct A {
    _inner: usize,
}

#[repr(transparent)]
pub struct B {
    inner: [A],
}

impl B {
    /// Obtain a shared reference to an ADT within a slice
    #[requires(index < self.inner.len())]
    pub fn get(&self, index: usize) -> &A {
        //~^ ERROR Prusti encountered an unexpected internal error
        //~| NOTE We would appreciate a bug report
        //~| NOTE cannot generate fold-unfold Viper statements
        &self.inner[index]
    }

    /// Obtain a shared reference to an ADT within a slice
    #[pure]
    #[requires(index < self.inner.len())]
    pub const fn get_pure(&self, index: usize) -> &A {
        //~^ ERROR Prusti encountered an unexpected internal error
        //~| NOTE We would appreciate a bug report
        //~| NOTE cannot generate fold-unfold Viper statements
        &self.inner[index]
    }
}

fn main() {}
