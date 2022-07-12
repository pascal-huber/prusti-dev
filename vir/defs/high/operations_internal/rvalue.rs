use super::{
    super::ast::{
        expression::Expression,
        position::Position,
        rvalue::{visitors::RvalueWalker, OperandKind, Rvalue, Ref, Aggregate, Discriminant},
        ty::*,
        variable::*,
    },
    ty::Typed,
};

impl Ref {
    // pub fn get_lifetimes(&self) -> Vec<LifetimeConst>{
    //     // TODO: right?
    //     self.place_lifetimes.clone()
    // }
}

impl Aggregate {
    pub fn get_lifetimes(&self) -> Vec<LifetimeConst>{
        let mut lifetimes: Vec<LifetimeConst> = vec![];
        for operand in &self.operands {
            match operand.kind {
                OperandKind::Copy | OperandKind::Move => {
                    let operand_ty = operand.expression.get_type();
                    let operand_lifetimes = operand_ty.get_lifetimes();
                    lifetimes.extend(operand_lifetimes);
                }
                _ => {}
            }
        }
        let lifetimes_ty = self.ty.get_lifetimes();
        lifetimes.extend(lifetimes_ty);
        lifetimes
    }
}

impl Discriminant {
    pub fn get_lifetimes(&self) -> Vec<LifetimeConst>{
        self.place.get_lifetimes()
    }
}

impl Rvalue {
    pub fn check_no_default_position(&self) {
        struct Checker;
        impl RvalueWalker for Checker {
            fn walk_expression(&mut self, expression: &Expression) {
                expression.check_no_default_position();
            }
        }
        Checker.walk_rvalue(self)
    }

    pub fn get_lifetimes(&self) -> Vec<LifetimeConst> {
        match self {
            // TODO: add missing rvalue lifetime cases
            // Rvalue::Ref(reference) => {
            //     reference.get_lifetimes()
            // }
            Rvalue::Aggregate(value) => {
                value.get_lifetimes()
            }
            Rvalue::Discriminant(discriminant) => {
                discriminant.get_lifetimes()
            }
            _ => vec![],
        }
    }

    pub fn get_lifetimes_as_var(&self) -> Vec<VariableDecl> {
        let lifetimes_const = self.get_lifetimes();
        lifetimes_const
            .iter()
            .map(|lifetime| VariableDecl {
                name: lifetime.name.clone(),
                ty: Type::Lifetime,
            })
            .collect()
    }
}
