use super::{
    super::ast::{
        expression::Expression,
        position::Position,
        rvalue::{visitors::RvalueWalker, OperandKind, Rvalue},
        ty::*,
        variable::*,
    },
    ty::Typed,
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
    // TODO: add lifetimes from mir into Rvalues (same as Type)
    pub fn get_lifetimes(&self) -> Vec<LifetimeConst> {
        match self {
            // TODO: add missing rvalue lifetime cases
            Rvalue::Ref(reference) => {
                vec![reference.lifetime.clone()]
            }
            Rvalue::Aggregate(value) => {
                // TODO: not sure about this
                // value.ty.get_lifetimes()
                let mut lifetimes: Vec<LifetimeConst> = vec![];
                for operand in &value.operands {
                    match operand.kind {
                        OperandKind::Copy | OperandKind::Move => {
                            let operand_ty = operand.expression.get_type();
                            let operand_lifetimes = operand_ty.get_lifetimes();
                            lifetimes.extend(operand_lifetimes);
                        }
                        _ => {}
                    }
                }
                lifetimes
            }
            // Rvalue::Discriminant(discriminant) => {}
            //
            // something like this for discrimanant:
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
