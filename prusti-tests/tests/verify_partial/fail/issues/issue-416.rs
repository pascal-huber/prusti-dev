use prusti_contracts::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct SafeVec<T>(Vec<T>);

impl<T> SafeVec<T> {
    #[pure]
    // FIXME: The error is tracked in https://github.com/viperproject/prusti-dev/issues/683
    pub unsafe fn get_unchecked_1(&self, idx: usize) -> &T {
        //~^ ERROR Prusti encountered an unexpected internal error
        //~| NOTE We would appreciate a bug report
        //~| NOTE cannot generate fold-unfold Viper statements
        self.0.get_unchecked(idx) //~ ERROR use of impure function
    }

    #[pure]
    #[trusted]
    pub unsafe fn get_unchecked_2(&self, idx: usize) -> &T {
        self.0.get_unchecked(idx)
    }
}

fn main() {}
