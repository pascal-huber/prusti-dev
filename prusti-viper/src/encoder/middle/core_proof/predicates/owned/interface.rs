use crate::encoder::{
    errors::SpannedEncodingResult,
    middle::core_proof::{
        lifetimes::LifetimesInterface, lowerer::Lowerer, snapshots::SnapshotValidityInterface,
        type_layouts::TypeLayoutsInterface, types::TypesInterface,
    },
};
use rustc_hash::FxHashSet;

use crate::encoder::high::types::HighTypeEncoderInterface;
use vir_crate::{
    low::{self as vir_low},
    middle as vir_mid,
    middle::operations::ty::Typed,
};

use super::encoder::PredicateEncoder;

#[derive(Default)]
pub(in super::super) struct PredicatesOwnedState {
    unfolded_owned_non_aliased_predicates: FxHashSet<vir_mid::Type>,
}

pub(in super::super::super) trait PredicatesOwnedInterface {
    /// Marks that `OwnedNonAliased<ty>` was unfolded in the program and we need
    /// to provide its body.
    fn mark_owned_non_aliased_as_unfolded(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()>;
    fn collect_owned_predicate_decls(
        &mut self,
    ) -> SpannedEncodingResult<Vec<vir_low::PredicateDecl>>;
    fn acc_owned_non_aliased(
        &mut self,
        ty: &vir_mid::Type,
        place: impl Into<vir_low::Expression>,
        root_address: impl Into<vir_low::Expression>,
        snapshot: impl Into<vir_low::Expression>,
        lifetimes: Vec<impl Into<vir_low::Expression>>,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    fn extract_lifetime_arguments_from_rvalue(
        &mut self,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>>;
    fn anonymize_lifetimes(&mut self, lifetimes: &mut Vec<vir_low::VariableDecl>);
    fn extract_lifetime_arguments_from_type(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>>;
    fn extract_non_type_arguments_from_type(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>>;
    fn extract_non_type_arguments_from_type_excluding_lifetimes(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>>;
    fn extract_non_type_parameters_from_type(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>>;
    fn extract_non_type_parameters_from_type_as_exprs(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>>;
    fn extract_non_type_parameters_from_type_validity(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>>;
    /// FIXME: Array length should be per operand/target, and not just a global value.
    fn array_length_variable(&mut self) -> SpannedEncodingResult<vir_low::VariableDecl>;
}

impl<'p, 'v: 'p, 'tcx: 'v> PredicatesOwnedInterface for Lowerer<'p, 'v, 'tcx> {
    fn mark_owned_non_aliased_as_unfolded(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        if !self
            .predicates_encoding_state
            .owned
            .unfolded_owned_non_aliased_predicates
            .contains(ty)
        {
            self.ensure_type_definition(ty)?;
            self.predicates_encoding_state
                .owned
                .unfolded_owned_non_aliased_predicates
                .insert(ty.clone());
        }
        Ok(())
    }

    fn collect_owned_predicate_decls(
        &mut self,
    ) -> SpannedEncodingResult<Vec<vir_low::PredicateDecl>> {
        let unfolded_predicates = std::mem::take(
            &mut self
                .predicates_encoding_state
                .owned
                .unfolded_owned_non_aliased_predicates,
        );
        let mut predicate_encoder = PredicateEncoder::new(self, &unfolded_predicates);
        for ty in &unfolded_predicates {
            predicate_encoder.encode_owned_non_aliased(ty)?;
        }
        Ok(predicate_encoder.into_predicates())
    }

    fn acc_owned_non_aliased(
        &mut self,
        ty: &vir_mid::Type,
        place: impl Into<vir_low::Expression>,
        root_address: impl Into<vir_low::Expression>,
        snapshot: impl Into<vir_low::Expression>,
        lifetimes: Vec<impl Into<vir_low::Expression>>,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        use vir_low::macros::*;
        let mut arguments = vec![place.into(), root_address.into(), snapshot.into()];
        arguments.extend(lifetimes.into_iter().map(|lifetime| lifetime.into()));
        Ok(vir_low::Expression::predicate_access_predicate_no_pos(
            predicate_name! { OwnedNonAliased<ty> },
            arguments,
            vir_low::Expression::full_permission(),
        ))
    }

    fn extract_lifetime_arguments_from_rvalue(
        &mut self,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>> {
        let mut lifetimes: Vec<vir_low::VariableDecl> = vec![];
        if let vir_mid::Rvalue::Aggregate(value) = value {
            for operand in &value.operands {
                match operand.kind {
                    vir_mid::OperandKind::Copy | vir_mid::OperandKind::Move => {
                        let operand_ty = operand.expression.get_type();
                        if let vir_mid::ty::Type::Reference(reference) = operand_ty {
                            let lifetime = self
                                .encode_lifetime_const_into_variable(reference.lifetime.clone())?;
                            lifetimes.push(lifetime);
                        }
                    }
                    _ => {}
                }
            }
        } else if let vir_mid::Rvalue::Discriminant(vir_mid::ast::rvalue::Discriminant {
            place: vir_mid::Expression::Local(vir_mid::Local { variable, .. }),
        }) = value
        {
            let var_lifetimes = variable.ty.get_lifetimes();
            for lifetime_const in var_lifetimes {
                let lifetime = self.encode_lifetime_const_into_variable(lifetime_const)?;
                lifetimes.push(lifetime);
            }
        }
        Ok(lifetimes)
    }

    fn extract_lifetime_arguments_from_type(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>> {
        let mut lifetimes: Vec<vir_low::VariableDecl> = vec![];
        if ty.is_struct() {
            let type_decl = self.encoder.get_type_decl_mid(ty)?;
            if let vir_mid::TypeDecl::Struct(decl) = type_decl {
                for field in decl.iter_fields() {
                    if let vir_mid::Type::Reference(reference) = &field.ty {
                        let lifetime =
                            self.encode_lifetime_const_into_variable(reference.lifetime.clone())?;
                        lifetimes.push(lifetime)
                    }
                }
            }
        } else if ty.is_enum() {
            // let ty_lifetimes = ty.get_lifetimes();
            // for lifetime in ty_lifetimes {
            //     lifetimes.push(self.encode_lifetime_const_into_variable(lifetime.clone())?);
            // }
            let type_decl = self.encoder.get_type_decl_mid(ty)?;
            if let vir_mid::TypeDecl::Enum(decl) = type_decl {
                for (_discriminant, variant) in decl.discriminant_values.iter().zip(&decl.variants)
                {
                    for field in variant.fields.iter() {
                        if let vir_mid::Type::Reference(r) = &field.ty {
                            lifetimes.push(
                                self.encode_lifetime_const_into_variable(r.lifetime.clone())?,
                            );
                        }
                    }
                }
            } else if let vir_mid::TypeDecl::Struct(strct) = type_decl {
                for field in strct.iter_fields() {
                    if let vir_mid::Type::Reference(reference) = &field.ty {
                        let lifetime =
                            self.encode_lifetime_const_into_variable(reference.lifetime.clone())?;
                        lifetimes.push(lifetime)
                    }
                }
            }
        }
        Ok(lifetimes)
    }

    fn extract_non_type_arguments_from_type(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>> {
        let mut arguments = self.extract_lifetime_variables_as_expr(ty)?;
        if let vir_mid::Type::Array(ty) = ty {
            arguments.push(self.size_constant(ty.length)?);
        }
        Ok(arguments)
    }

    fn extract_non_type_arguments_from_type_excluding_lifetimes(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>> {
        if let vir_mid::Type::Array(ty) = ty {
            Ok(vec![self.size_constant(ty.length)?])
        } else {
            Ok(Vec::new())
        }
    }

    fn anonymize_lifetimes(&mut self, lifetimes: &mut Vec<vir_low::VariableDecl>) {
        for (i, lifetime) in lifetimes.iter_mut().enumerate() {
            lifetime.name = format!("lft_{}", i);
        }
    }

    fn extract_non_type_parameters_from_type(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>> {
        // FIXME: Figure out how to avoid these magic variable names.
        let parameters = match ty {
            vir_mid::Type::Reference(_) => {
                use vir_low::macros::*;
                vec![var! { lifetime: Lifetime }]
            }
            vir_mid::Type::Array(_) => {
                vec![self.array_length_variable()?]
            }
            _ => Vec::new(),
        };
        Ok(parameters)
    }

    fn extract_non_type_parameters_from_type_as_exprs(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>> {
        let parameters = self.extract_non_type_parameters_from_type(ty)?;
        Ok(parameters
            .into_iter()
            .map(|parameter| parameter.into())
            .collect())
    }

    fn extract_non_type_parameters_from_type_validity(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>> {
        let validity_calls = match ty {
            vir_mid::Type::Array(_) => {
                let variable = self.array_length_variable()?;
                let size_type = &self.size_type_mid()?;
                vec![self.encode_snapshot_valid_call_for_type(variable.into(), size_type)?]
            }
            _ => Vec::new(),
        };
        Ok(validity_calls)
    }

    fn array_length_variable(&mut self) -> SpannedEncodingResult<vir_low::VariableDecl> {
        Ok(vir_low::VariableDecl::new(
            "array_length",
            self.size_type()?,
        ))
    }
}
