use super::super::ast::{
    expression::Expression,
    position::Position,
    rvalue::{visitors::RvalueWalker, Rvalue, OperandKind},
    ty::*,
    variable::*,
};

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
    // let mut lifetimes: Vec<vir_low::VariableDecl> = vec![];
    // if let vir_mid::Rvalue::Aggregate(value) = value {
    //     for operand in &value.operands {
    //         match operand.kind {
    //             vir_mid::OperandKind::Copy | vir_mid::OperandKind::Move => {
    //                 let operand_ty = operand.expression.get_type();
    //                 if let vir_mid::ty::Type::Reference(reference) = operand_ty {
    //                     let lifetime = self
    //                         .encode_lifetime_const_into_variable(reference.lifetime.clone())?;
    //                     lifetimes.push(lifetime);
    //                 }
    //             }
    //             _ => {}
    //         }
    //     }
    // } else if let vir_mid::Rvalue::Discriminant(vir_mid::ast::rvalue::Discriminant {
    //     place: vir_mid::Expression::Local(vir_mid::Local { variable, .. }),
    // }) = value
    // {
    //     let var_lifetimes = variable.ty.get_lifetimes();
    //     for lifetime_const in var_lifetimes {
    //         let lifetime = self.encode_lifetime_const_into_variable(lifetime_const)?;
    //         lifetimes.push(lifetime);
    //     }
    // }
    // Ok(lifetimes)
    // TODO: add lifetimes from mir into Rvalues (same as Type)
    pub fn get_lifetimes(&self) -> Vec<LifetimeConst> {
        match self {
            Rvalue::Ref(reference) => {
                vec![reference.lifetime.clone()]
            }
            // Rvalue::Aggregate(value) => {
            //     let mut lifetimes: Vec<LifetimeConst> = vec![];
            //     for operand in &value.operands {
            //         match operand.kind {
            //             OperandKind::Copy | OperandKind::Move => {
            //                 let operand_ty = operand.expression.get_type();
            //                 if let ty::Type::Reference(reference) = operand_ty {
            //                     let lifetime = self
            //                         .encode_lifetime_const_into_variable(reference.lifetime.clone())?;
            //                     lifetimes.push(lifetime);
            //                 }
            //             }
            //             _ => {}
            //         }
            //     }
            //     lifetimes
            // }
            // Rvalue::Discriminant(discriminant) => {
            //
            // }
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

    // pub fn get_lifetimes_as_var(&self) -> Vec<VariableDecl> {
    //     let lifetimes_const = self.get_lifetimes();
    //     lifetimes_const
    //         .iter()
    //         .map(|lifetime| VariableDecl {
    //             name: lifetime.name.clone(),
    //             ty: Type::Lifetime,
    //         })
    //         .collect()
    // }
}
