use rustc_middle::ty;

pub fn opaque_lifetime_string(index: usize) -> String {
    format!("bw{}", index)
}

pub trait LifetimeString {
    fn lifetime_string(&self) -> String;
}

impl LifetimeString for ty::RegionVid {
    fn lifetime_string(&self) -> String {
        opaque_lifetime_string(self.index())
    }
}

impl<'tcx> LifetimeString for ty::Region<'tcx> {
    fn lifetime_string(&self) -> String {
        match self.kind() {
            ty::ReEarlyBound(_) => {
                unimplemented!("ReEarlyBound: {}", format!("{}", self));
            },
            ty::ReLateBound(_, _) => {
                unimplemented!("ReLateBound: {}", format!("{}", self));
            },
            ty::ReFree(_) => {
                unimplemented!("ReFree: {}", format!("{}", self));
            },
            ty::ReStatic=> String::from("lft_static"),
            ty::ReVar(region_vid) => format!("lft_{}", region_vid.index()),
            ty::RePlaceholder(_) => {
                unimplemented!("RePlaceholder: {}", format!("{}", self));
            },
            ty::ReEmpty(_) => {
                unimplemented!("ReEmpty: {}", format!("{}", self));
            },
            ty::ReErased => String::from("lft_erased"),
        }
    }
}


// impl LifetimeString for Loan {
//     fn lifetime_string(&self) -> String {
//         format!("bw{}", self.index())
//     }
// }

// impl<T: rustc_index::vec::Idx> LifetimeString for T {
//     fn lifetime_string(&self) -> String {
//         String::from("asdf")
//     }
// }


// pub fn get_lifetime_string(x: Atom) {
//
// }
