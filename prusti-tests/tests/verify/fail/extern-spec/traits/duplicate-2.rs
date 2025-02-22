extern crate prusti_contracts;
use prusti_contracts::*;

#[extern_spec]
trait Iterator {
    type Item;

    #[requires(true)]
    fn next(&mut self) -> Option<Self::Item>;
}

#[extern_spec]
trait Iterator { 
    type Item;

    #[requires(true)] //~ ERROR: duplicate specification for std::iter::Iterator::next
    fn next(&mut self) -> Option<Self::Item>;
}

fn main() {

}