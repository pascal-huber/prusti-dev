use crate::encoder::{
    errors::{BuiltinMethodKind, ErrorCtxt, SpannedEncodingResult},
    high::types::HighTypeEncoderInterface,
    middle::core_proof::{
        addresses::AddressesInterface,
        builtin_methods::split_join::SplitJoinHelper,
        compute_address::ComputeAddressInterface,
        errors::ErrorsInterface,
        fold_unfold::FoldUnfoldInterface,
        lifetimes::LifetimesInterface,
        lowerer::{
            Lowerer, MethodsLowererInterface, PredicatesLowererInterface, VariablesLowererInterface,
        },
        places::PlacesInterface,
        predicates::{PredicatesMemoryBlockInterface, PredicatesOwnedInterface},
        references::ReferencesInterface,
        snapshots::{
            BuiltinFunctionsInterface, IntoProcedureFinalSnapshot, IntoProcedureSnapshot,
            IntoSnapshot, SnapshotBytesInterface, SnapshotValidityInterface,
            SnapshotValuesInterface, SnapshotVariablesInterface,
        },
        type_layouts::TypeLayoutsInterface,
        utils::type_decl_encoder::TypeDeclWalker,
    },
};
use rustc_hash::FxHashSet;
use vir_crate::{
    common::{
        expression::{ExpressionIterator, UnaryOperationHelpers},
        identifier::WithIdentifier,
    },
    low::{self as vir_low, macros::method_name},
    middle::{self as vir_mid, operations::ty::Typed},
};

#[derive(Default)]
pub(in super::super) struct BuiltinMethodsState {
    encoded_write_place_methods: FxHashSet<vir_mid::Type>,
    encoded_move_place_methods: FxHashSet<vir_mid::Type>,
    encoded_copy_place_methods: FxHashSet<vir_mid::Type>,
    encoded_write_address_methods: FxHashSet<vir_mid::Type>,
    encoded_owned_non_aliased_havoc_methods: FxHashSet<vir_mid::Type>,
    encoded_memory_block_split_methods: FxHashSet<vir_mid::Type>,
    encoded_memory_block_join_methods: FxHashSet<vir_mid::Type>,
    encoded_memory_block_havoc_methods: FxHashSet<vir_mid::Type>,
    encoded_into_memory_block_methods: FxHashSet<vir_mid::Type>,
    encoded_assign_methods: FxHashSet<String>,
    encoded_consume_operand_methods: FxHashSet<String>,
    encoded_newlft_method: bool,
    encoded_endlft_method: bool,
    encoded_open_frac_bor_atomic_methods: FxHashSet<vir_mid::Type>,
    encoded_dead_inclusion_method: bool,
    encoded_lft_tok_sep_take_methods: FxHashSet<usize>,
    encoded_lft_tok_sep_return_methods: FxHashSet<usize>,
    encoded_open_close_mut_ref_methods: FxHashSet<vir_mid::Type>,
    encoded_bor_shorten_methods: FxHashSet<vir_mid::Type>,
}

trait Private {
    fn encode_rvalue_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<()>;
    fn encode_operand_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<()>;
    fn encode_place_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        expression: &vir_mid::Expression,
    ) -> SpannedEncodingResult<()>;
    fn encode_open_frac_bor_atomic_method_name(
        &self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<String>;
    fn encode_lft_tok_sep_take_method_name(
        &self,
        lft_count: usize,
    ) -> SpannedEncodingResult<String>;
    fn encode_lft_tok_sep_return_method_name(
        &self,
        lft_count: usize,
    ) -> SpannedEncodingResult<String>;
    fn encode_assign_method_name(
        &self,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<String>;
    fn encode_consume_operand_method_name(
        &self,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<String>;
    fn encode_havoc_owned_non_aliased_method_name(
        &self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<String>;
    fn encode_assign_method(
        &mut self,
        method_name: &str,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<()>;
    fn encode_consume_operand_method(
        &mut self,
        method_name: &str,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        value: &vir_mid::Rvalue,
        result_type: &vir_mid::Type,
        result_value: &vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue_checked_binary_op(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        value: &vir_mid::ast::rvalue::CheckedBinaryOp,
        ty: &vir_mid::Type,
        result_value: &vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue_reborrow(
        &mut self,
        method_name: &str,
        parameters: Vec<vir_low::VariableDecl>,
        pres: Vec<vir_low::Expression>,
        posts: Vec<vir_low::Expression>,
        value: &vir_mid::ast::rvalue::Reborrow,
        ty: &vir_mid::Type,
        result_value: vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue_ref(
        &mut self,
        method_name: &str,
        parameters: Vec<vir_low::VariableDecl>,
        pres: Vec<vir_low::Expression>,
        posts: Vec<vir_low::Expression>,
        value: &vir_mid::ast::rvalue::Ref,
        ty: &vir_mid::Type,
        result_value: vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_operand(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: Option<&mut Vec<vir_low::Statement>>,
        operand_counter: u32,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<vir_low::VariableDecl>;
    fn encode_assign_operand_place(
        &mut self,
        operand_counter: u32,
    ) -> SpannedEncodingResult<vir_low::VariableDecl>;
    fn encode_assign_operand_address(
        &mut self,
        operand_counter: u32,
    ) -> SpannedEncodingResult<vir_low::VariableDecl>;
    fn encode_assign_operand_snapshot(
        &mut self,
        operand_counter: u32,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<vir_low::VariableDecl>;
}

impl<'p, 'v: 'p, 'tcx: 'v> Private for Lowerer<'p, 'v, 'tcx> {
    fn encode_rvalue_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<()> {
        let lifetimes_const = value.get_lifetimes();
        let mut lifetimes = vec![];
        for lifetime in lifetimes_const {
            let lft: vir_low::Expression =
                self.encode_lifetime_const_into_variable(lifetime)?.into();
            lifetimes.push(lft);
        }
        match value {
            vir_mid::Rvalue::Repeat(value) => {
                self.encode_operand_arguments(arguments, &value.argument)?;
                arguments.push(value.count.into());
            }
            vir_mid::Rvalue::Reborrow(value) => {
                let place_lifetime =
                    self.encode_lifetime_const_into_variable(value.place_lifetime.clone())?;
                let operand_lifetime =
                    self.encode_lifetime_const_into_variable(value.operand_lifetime.clone())?;
                let perm_amount = value
                    .lifetime_token_permission
                    .to_procedure_snapshot(self)?;
                arguments.push(place_lifetime.into());
                self.encode_place_arguments(arguments, &value.place)?;
                if value.is_mut {
                    let value_final = value.place.to_procedure_final_snapshot(self)?;
                    arguments.push(value_final);
                }
                arguments.push(operand_lifetime.into());
                arguments.push(perm_amount);
            }
            vir_mid::Rvalue::Ref(value) => {
                self.encode_place_arguments(arguments, &value.place)?;
                let lifetime = self.encode_lifetime_const_into_variable(value.lifetime.clone())?;
                arguments.push(lifetime.into());
                let perm_amount = value
                    .lifetime_token_permission
                    .to_procedure_snapshot(self)?;
                arguments.push(perm_amount);
            }
            vir_mid::Rvalue::AddressOf(value) => {
                self.encode_place_arguments(arguments, &value.place)?;
            }
            vir_mid::Rvalue::Len(value) => {
                self.encode_place_arguments(arguments, &value.place)?;
                arguments
                    .extend(self.extract_non_type_arguments_from_type(value.place.get_type())?);
            }
            vir_mid::Rvalue::UnaryOp(value) => {
                self.encode_operand_arguments(arguments, &value.argument)?;
            }
            vir_mid::Rvalue::BinaryOp(value) => {
                self.encode_operand_arguments(arguments, &value.left)?;
                self.encode_operand_arguments(arguments, &value.right)?;
            }
            vir_mid::Rvalue::CheckedBinaryOp(value) => {
                self.encode_operand_arguments(arguments, &value.left)?;
                self.encode_operand_arguments(arguments, &value.right)?;
            }
            vir_mid::Rvalue::Discriminant(value) => {
                self.encode_place_arguments(arguments, &value.place)?;
            }
            vir_mid::Rvalue::Aggregate(value) => {
                for operand in &value.operands {
                    self.encode_operand_arguments(arguments, operand)?;
                }
                arguments.extend(lifetimes);
            }
        }
        Ok(())
    }
    fn encode_operand_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<()> {
        match operand.kind {
            vir_mid::OperandKind::Copy | vir_mid::OperandKind::Move => {
                self.encode_place_arguments(arguments, &operand.expression)?;
            }
            vir_mid::OperandKind::Constant => {
                arguments.push(operand.expression.to_procedure_snapshot(self)?);
            }
        }
        Ok(())
    }
    fn encode_place_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        expression: &vir_mid::Expression,
    ) -> SpannedEncodingResult<()> {
        arguments.push(self.encode_expression_as_place(expression)?);
        arguments.push(self.extract_root_address(expression)?);
        arguments.push(expression.to_procedure_snapshot(self)?);
        Ok(())
    }
    fn encode_open_frac_bor_atomic_method_name(
        &self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<String> {
        Ok(format!("frac_bor_atomic_acc${}", ty.get_identifier()))
    }
    fn encode_lft_tok_sep_take_method_name(
        &self,
        lft_count: usize,
    ) -> SpannedEncodingResult<String> {
        Ok(format!("lft_tok_sep_take${}", lft_count))
    }
    fn encode_lft_tok_sep_return_method_name(
        &self,
        lft_count: usize,
    ) -> SpannedEncodingResult<String> {
        Ok(format!("lft_tok_sep_return${}", lft_count))
    }
    fn encode_assign_method_name(
        &self,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<String> {
        if let vir_mid::Rvalue::Reborrow(_reborrow) = &value {
            Ok(format!(
                "reborrow${}${}",
                ty.get_identifier(),
                value.get_identifier()
            ))
        } else {
            Ok(format!(
                "assign${}${}",
                ty.get_identifier(),
                value.get_identifier()
            ))
        }
    }
    fn encode_consume_operand_method_name(
        &self,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<String> {
        Ok(format!("consume${}", operand.get_identifier()))
    }
    fn encode_havoc_owned_non_aliased_method_name(
        &self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<String> {
        Ok(format!("havoc_owned${}", ty.get_identifier()))
    }
    fn encode_assign_method(
        &mut self,
        method_name: &str,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_assign_methods
            .contains(method_name)
        {
            self.encode_compute_address(ty)?;
            self.encode_write_place_method(ty)?;
            let span = self.encoder.get_type_definition_span_mid(ty)?;
            let position = self.register_error(
                span,
                ErrorCtxt::UnexpectedBuiltinMethod(BuiltinMethodKind::MovePlace),
            );
            use vir_low::macros::*;
            let compute_address = ty!(Address);
            let size_of = self.encode_type_size_expression(ty)?;
            var_decls! {
                target_place: Place,
                target_address: Address
            };
            let mut parameters = vec![target_place.clone(), target_address.clone()];
            var_decls! { result_value: {ty.to_snapshot(self)?} };
            let mut pres = vec![
                expr! { acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_address)), [size_of])) },
            ];
            let mut posts = Vec::new();
            let mut pre_write_statements = Vec::new();
            let mut post_write_statements = Vec::new();
            let mut encode_body = true;
            let lifetimes_rvalue = self.extract_lifetime_arguments_from_rvalue_anonymise(value)?;
            match value {
                vir_mid::Rvalue::CheckedBinaryOp(value) => {
                    self.encode_assign_method_rvalue_checked_binary_op(
                        &mut parameters,
                        &mut pres,
                        &mut posts,
                        &mut pre_write_statements,
                        value,
                        ty,
                        &result_value,
                        position,
                    )?;
                }
                vir_mid::Rvalue::Reborrow(value) => {
                    self.encode_assign_method_rvalue_reborrow(
                        method_name,
                        parameters,
                        pres,
                        posts,
                        value,
                        ty,
                        result_value,
                        position,
                    )?;
                    return Ok(());
                }
                vir_mid::Rvalue::Ref(value) => {
                    self.encode_assign_method_rvalue_ref(
                        method_name,
                        parameters,
                        pres,
                        posts,
                        value,
                        ty,
                        result_value,
                        position,
                    )?;
                    return Ok(());
                }
                _ => {
                    let args = self.extract_non_type_parameters_from_type_as_exprs(ty)?;
                    let mut args2 = args.clone();
                    post_write_statements.push(stmtp! {
                        position => call write_place<ty>(target_place, target_address, result_value; args)
                    });
                    let lifetimes: Vec<vir_low::Expression> = self
                        .extract_lifetime_arguments_from_rvalue_anonymise(value)?
                        .iter()
                        .cloned()
                        .map(|x| x.into())
                        .collect();
                    args2.extend(lifetimes);
                    // FIXME: body is not encoded if we have additional lifetime
                    // parameters from structs.
                    // FIXME: As a workaround for #1065, we encode bodies only
                    // of types that do not contain generic bodies.
                    encode_body = lifetimes_rvalue.is_empty() && !ty.contains_type_variables();
                    posts.push(
                        expr! { acc(OwnedNonAliased<ty>(target_place, target_address, result_value; args2)) },
                    );
                    let old_pres = std::mem::replace(
                        &mut pres,
                        self.extract_non_type_parameters_from_type_validity(ty)?,
                    );
                    pres.extend(old_pres);
                    self.encode_assign_method_rvalue(
                        &mut parameters,
                        &mut pres,
                        &mut posts,
                        &mut pre_write_statements,
                        value,
                        ty,
                        &result_value,
                        position,
                    )?;
                    parameters.extend(self.extract_non_type_parameters_from_type(ty)?);
                    parameters.extend(lifetimes_rvalue);
                }
            }
            let mut statements = pre_write_statements;
            statements.extend(post_write_statements);
            let body = if encode_body { Some(statements) } else { None };
            let method = vir_low::MethodDecl::new(
                method_name,
                parameters,
                vec![result_value],
                pres,
                posts,
                body,
            );
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_assign_methods
                .insert(method_name.to_string());
        }
        Ok(())
    }
    fn encode_consume_operand_method(
        &mut self,
        method_name: &str,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_consume_operand_methods
            .contains(method_name)
        {
            let mut parameters = Vec::new();
            let mut pres = Vec::new();
            let mut posts = Vec::new();
            self.encode_assign_operand(&mut parameters, &mut pres, &mut posts, None, 1, operand)?;
            let ty = operand.expression.get_type();
            let lifetimes_ty = self.extract_lifetime_variables_anonymise(ty)?;
            parameters.extend(lifetimes_ty);
            let method =
                vir_low::MethodDecl::new(method_name, parameters, Vec::new(), pres, posts, None);
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_consume_operand_methods
                .insert(method_name.to_string());
        }
        Ok(())
    }
    fn encode_assign_method_rvalue(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        value: &vir_mid::Rvalue,
        result_type: &vir_mid::Type,
        result_value: &vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        use vir_low::macros::*;
        let assigned_value = match value {
            vir_mid::Rvalue::Repeat(value) => {
                let operand_value = self.encode_assign_operand(
                    parameters,
                    pres,
                    posts,
                    Some(pre_write_statements),
                    1,
                    &value.argument,
                )?;
                var_decls! { count: Int };
                parameters.push(count.clone());
                pres.push(expr! { [0.into()] <= count });
                self.encode_sequence_repeat_constructor_call(
                    result_type,
                    operand_value.into(),
                    count.into(),
                )?
            }
            vir_mid::Rvalue::Reborrow(_value) => {
                unreachable!("Reborrow should be handled in the caller.");
            }
            vir_mid::Rvalue::Ref(_value) => {
                unreachable!("Ref should be handled in the caller.");
            }
            vir_mid::Rvalue::AddressOf(value) => {
                let ty = value.place.get_type();
                var_decls! {
                    operand_place: Place,
                    operand_address: Address,
                    operand_value: { ty.to_snapshot(self)? }
                };
                let predicate = expr! {
                    acc(OwnedNonAliased<ty>(operand_place, operand_address, operand_value))
                };
                let compute_address = ty!(Address);
                let address =
                    expr! { ComputeAddress::compute_address(operand_place, operand_address) };
                pres.push(predicate.clone());
                posts.push(predicate);
                parameters.push(operand_place);
                parameters.push(operand_address);
                parameters.push(operand_value);
                self.construct_constant_snapshot(result_type, address, position)?
            }
            vir_mid::Rvalue::Len(value) => {
                let ty = value.place.get_type();
                var_decls! {
                    operand_place: Place,
                    operand_address: Address,
                    operand_value: { ty.to_snapshot(self)? }
                };
                let args = self.extract_non_type_parameters_from_type_as_exprs(ty)?;
                let predicate = expr! {
                    acc(OwnedNonAliased<ty>(operand_place, operand_address, operand_value; args))
                };
                parameters.push(operand_place);
                parameters.push(operand_address);
                parameters.push(operand_value);
                parameters.extend(self.extract_non_type_parameters_from_type(ty)?);
                pres.extend(self.extract_non_type_parameters_from_type_validity(ty)?);
                pres.push(predicate.clone());
                posts.push(predicate);
                self.array_length_variable()?.into()
            }
            vir_mid::Rvalue::UnaryOp(value) => {
                let operand_value = self.encode_assign_operand(
                    parameters,
                    pres,
                    posts,
                    Some(pre_write_statements),
                    1,
                    &value.argument,
                )?;
                self.construct_unary_op_snapshot(
                    value.kind,
                    value.argument.expression.get_type(),
                    operand_value.into(),
                    position,
                )?
            }
            vir_mid::Rvalue::BinaryOp(value) => {
                let operand_left = self.encode_assign_operand(
                    parameters,
                    pres,
                    posts,
                    Some(pre_write_statements),
                    1,
                    &value.left,
                )?;
                let operand_right = self.encode_assign_operand(
                    parameters,
                    pres,
                    posts,
                    Some(pre_write_statements),
                    2,
                    &value.right,
                )?;
                self.construct_binary_op_snapshot(
                    value.kind,
                    value.kind.get_result_type(value.left.expression.get_type()),
                    value.left.expression.get_type(),
                    operand_left.into(),
                    operand_right.into(),
                    position,
                )?
            }
            vir_mid::Rvalue::CheckedBinaryOp(_) => {
                unreachable!("CheckedBinaryOp should be handled in the caller.");
            }
            vir_mid::Rvalue::Discriminant(value) => {
                let ty = value.place.get_type();
                var_decls! {
                    operand_place: Place,
                    operand_address: Address,
                    operand_value: { ty.to_snapshot(self)? }
                };
                let lifetimes = self.extract_lifetime_variables_anonymise(ty)?;
                let lifetime_exprs: Vec<vir_low::Expression> = lifetimes
                    .iter()
                    .cloned()
                    .map(|lifetime| lifetime.into())
                    .collect();
                let predicate = expr! {
                    acc(OwnedNonAliased<ty>(operand_place, operand_address, operand_value; lifetime_exprs))
                };
                pres.push(predicate.clone());
                posts.push(predicate);
                parameters.push(operand_place);
                parameters.push(operand_address);
                parameters.push(operand_value.clone());
                pres.push(
                    self.encode_snapshot_valid_call_for_type(operand_value.clone().into(), ty)?,
                );
                let discriminant_call =
                    self.obtain_enum_discriminant(operand_value.into(), ty, position)?;
                self.construct_constant_snapshot(result_type, discriminant_call, position)?
            }
            vir_mid::Rvalue::Aggregate(value) => {
                let mut arguments = Vec::new();
                for (i, operand) in value.operands.iter().enumerate() {
                    // FIXME: As a workaround for #1065, we encode bodies only of
                    // types that do not contain generic bodies.
                    let pre_write_statements = if result_type.contains_type_variables() {
                        None
                    } else {
                        Some(&mut *pre_write_statements)
                    };
                    let operand_value = self.encode_assign_operand(
                        parameters,
                        pres,
                        posts,
                        pre_write_statements,
                        i.try_into().unwrap(),
                        operand,
                    )?;
                    arguments.push(operand_value.into());
                }
                let assigned_value = match &value.ty {
                    vir_mid::Type::Union(_) | vir_mid::Type::Enum(_) => {
                        let variant_constructor =
                            self.construct_struct_snapshot(&value.ty, arguments, position)?;
                        self.construct_enum_snapshot(&value.ty, variant_constructor, position)?
                    }
                    vir_mid::Type::Struct(_) | vir_mid::Type::Tuple(_) => {
                        self.construct_struct_snapshot(&value.ty, arguments, position)?
                    }
                    vir_mid::Type::Array(value_ty) => vir_low::Expression::seq(
                        value_ty.element_type.to_snapshot(self)?,
                        arguments,
                        position,
                    ),
                    ty => unimplemented!("{}", ty),
                };
                posts.push(
                    self.encode_snapshot_valid_call_for_type(assigned_value.clone(), result_type)?,
                );
                assigned_value
            }
        };
        posts.push(exprp! { position => result_value == [assigned_value.clone()]});
        posts.push(
            self.encode_snapshot_valid_call_for_type(result_value.clone().into(), result_type)?,
        );
        pre_write_statements.push(vir_low::Statement::assign(
            result_value.clone(),
            assigned_value,
            position,
        ));
        Ok(())
    }
    fn encode_assign_method_rvalue_checked_binary_op(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        value: &vir_mid::ast::rvalue::CheckedBinaryOp,
        ty: &vir_mid::Type,
        result_value: &vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        // In case the operation fails, the state of the assigned value
        // is unknown.
        use vir_low::macros::*;
        var_decls! {
            target_place: Place,
            target_address: Address
        };
        let compute_address = ty!(Address);
        let type_decl = self.encoder.get_type_decl_mid(ty)?.unwrap_tuple();
        let (operation_result_field, flag_field) = {
            let mut iter = type_decl.iter_fields();
            (iter.next().unwrap(), iter.next().unwrap())
        };
        let flag_place =
            self.encode_field_place(ty, &flag_field, target_place.clone().into(), position)?;
        let flag_value = self.obtain_struct_field_snapshot(
            ty,
            &flag_field,
            result_value.clone().into(),
            position,
        )?;
        let result_address =
            expr! { (ComputeAddress::compute_address(target_place, target_address)) };
        let operation_result_address = self.encode_field_address(
            ty,
            &operation_result_field,
            result_address.clone(),
            position,
        )?;
        let operation_result_value = self.obtain_struct_field_snapshot(
            ty,
            &operation_result_field,
            result_value.clone().into(),
            position,
        )?;
        let flag_type = &vir_mid::Type::Bool;
        let operation_result_type = value.kind.get_result_type(value.left.expression.get_type());
        let size_of_result = self.encode_type_size_expression(operation_result_type)?;
        let post_write_statements = vec![
            stmtp! { position =>
                call memory_block_split<ty>([result_address], [vir_low::Expression::full_permission()])
            },
            stmtp! { position =>
                call write_address<operation_result_type>([operation_result_address.clone()], [operation_result_value.clone()])
            },
            stmtp! { position =>
                call write_place<flag_type>([flag_place.clone()], target_address, [flag_value.clone()])
            },
        ];
        posts.push(
            expr! { acc(MemoryBlock([operation_result_address.clone()], [size_of_result.clone()])) },
        );
        posts.push(
            expr! { acc(OwnedNonAliased<flag_type>([flag_place], target_address, [flag_value.clone()])) },
        );
        let operand_left = self.encode_assign_operand(
            parameters,
            pres,
            posts,
            Some(pre_write_statements),
            1,
            &value.left,
        )?;
        let operand_right = self.encode_assign_operand(
            parameters,
            pres,
            posts,
            Some(pre_write_statements),
            2,
            &value.right,
        )?;
        let operation_result = self.construct_binary_op_snapshot(
            value.kind,
            operation_result_type,
            value.left.expression.get_type(),
            operand_left.into(),
            operand_right.into(),
            position,
        )?;
        let validity = self
            .encode_snapshot_valid_call_for_type(operation_result.clone(), operation_result_type)?;
        let flag_result = self.construct_constant_snapshot(
            &vir_mid::Type::Bool,
            vir_low::Expression::not(validity.clone()),
            position,
        )?;
        pre_write_statements.push(vir_low::Statement::comment(
            "Viper does not support ADT assignments, so we use an assume.".to_string(),
        ));
        let operation_result_value_condition = expr! {
            [validity] ==> ([operation_result_value.clone()] == [operation_result])
        };
        pre_write_statements.push(stmtp! { position =>
            assume ([operation_result_value_condition.clone()])
        });
        let flag_value_condition = expr! {
            [flag_value] == [flag_result]
        };
        pre_write_statements.push(stmtp! { position =>
            assume ([flag_value_condition.clone()])
        });
        pre_write_statements.extend(post_write_statements);
        posts.push(operation_result_value_condition);
        posts.push(flag_value_condition);
        let bytes =
            self.encode_memory_block_bytes_expression(operation_result_address, size_of_result)?;
        let to_bytes = ty! { Bytes };
        posts.push(expr! {
            [bytes] == (Snap<operation_result_type>::to_bytes([operation_result_value]))
        });
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue_reborrow(
        &mut self,
        method_name: &str,
        mut parameters: Vec<vir_low::VariableDecl>,
        mut pres: Vec<vir_low::Expression>,
        mut posts: Vec<vir_low::Expression>,
        value: &vir_mid::ast::rvalue::Reborrow,
        result_type: &vir_mid::Type,
        result_value: vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        use vir_low::macros::*;
        let reference_type = result_type.clone().unwrap_reference();
        let ty = value.place.get_type();
        var_decls! {
            target_place: Place,
            target_address: Address,
            old_lifetime: Lifetime,
            operand_place: Place,
            operand_address: Address,
            operand_value_current: { ty.to_snapshot(self)? },
            operand_value_final: { ty.to_snapshot(self)? }, // use only for unique references
            operand_lifetime: Lifetime,
            lifetime_perm: Perm
        };
        let predicate = if reference_type.uniqueness.is_unique() {
            expr! {
                acc(UniqueRef<ty>(
                    old_lifetime,
                    [operand_place.clone().into()],
                    [operand_address.clone().into()],
                    [operand_value_current.clone().into()],
                    [operand_value_final.clone().into()])
                )
            }
        } else {
            expr! {
                acc(FracRef<ty>(
                    old_lifetime,
                    [operand_place.clone().into()],
                    [operand_address.clone().into()],
                    [operand_value_current.clone().into()])
                )
            }
        };
        let valid_result =
            self.encode_snapshot_valid_call_for_type(result_value.clone().into(), result_type)?;
        let reference_predicate = expr! {
            (acc(OwnedNonAliased<result_type>(target_place, target_address, result_value, operand_lifetime))) &&
            [valid_result]
        };
        let lifetime_token =
            self.encode_lifetime_token(operand_lifetime.clone(), lifetime_perm.clone().into())?;
        let restoration = {
            let ty_value = &value.place.get_type().clone();
            let final_snapshot = self.reference_target_final_snapshot(
                result_type,
                result_value.clone().into(),
                position,
            )?;
            let validity = self.encode_snapshot_valid_call_for_type(final_snapshot, ty)?;
            if reference_type.uniqueness.is_unique() {
                expr! {
                    wand(
                        (acc(DeadLifetimeToken(operand_lifetime))) --* (
                            (acc(UniqueRef<ty_value>(
                                old_lifetime,
                                [operand_place.clone().into()],
                                [operand_address.clone().into()],
                                [operand_value_current.clone().into()],
                                [operand_value_final.clone().into()])
                            )) &&
                            [validity] &&
                            // DeadLifetimeToken is duplicable and does not get consumed.
                            (acc(DeadLifetimeToken(operand_lifetime)))
                        )
                    )
                }
            } else {
                predicate.clone()
            }
        };
        let reference_target_address =
            self.reference_address(result_type, result_value.clone().into(), position)?;
        posts.push(expr! {
            operand_address == [reference_target_address]
        });
        let reference_target_current_snapshot = self.reference_target_current_snapshot(
            result_type,
            result_value.clone().into(),
            position,
        )?;
        posts.push(expr! {
            operand_value_current == [reference_target_current_snapshot]
        });
        let reference_target_final_snapshot = self.reference_target_final_snapshot(
            result_type,
            result_value.clone().into(),
            position,
        )?;
        if reference_type.uniqueness.is_unique() {
            posts.push(expr! {
                operand_value_final == [reference_target_final_snapshot]
            });
        }
        pres.push(expr! {
            [vir_low::Expression::no_permission()] < lifetime_perm
        });
        pres.push(expr! {
            lifetime_perm < [vir_low::Expression::full_permission()]
        });
        pres.push(predicate);
        pres.push(lifetime_token.clone());
        posts.push(lifetime_token);
        posts.push(reference_predicate);
        posts.push(restoration);
        parameters.push(old_lifetime);
        parameters.push(operand_place);
        parameters.push(operand_address);
        parameters.push(operand_value_current);
        if reference_type.uniqueness.is_unique() {
            parameters.push(operand_value_final);
        }
        parameters.push(operand_lifetime);
        parameters.push(lifetime_perm);
        let method = vir_low::MethodDecl::new(
            method_name,
            parameters,
            vec![result_value],
            pres,
            posts,
            None,
        );
        self.declare_method(method)?;
        self.builtin_methods_state
            .encoded_assign_methods
            .insert(method_name.to_string());
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue_ref(
        &mut self,
        method_name: &str,
        mut parameters: Vec<vir_low::VariableDecl>,
        mut pres: Vec<vir_low::Expression>,
        mut posts: Vec<vir_low::Expression>,
        value: &vir_mid::ast::rvalue::Ref,
        result_type: &vir_mid::Type,
        result_value: vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        use vir_low::macros::*;
        let ty = value.place.get_type();
        var_decls! {
            target_place: Place,
            target_address: Address,
            operand_place: Place,
            operand_address: Address,
            operand_value: { ty.to_snapshot(self)? },
            operand_lifetime: Lifetime,
            lifetime_perm: Perm
        };
        let predicate = expr! {
            acc(OwnedNonAliased<ty>(operand_place, operand_address, operand_value))
        };
        let reference_predicate = expr! {
            acc(OwnedNonAliased<result_type>(target_place, target_address, result_value, operand_lifetime))
        };
        let lifetime_token =
            self.encode_lifetime_token(operand_lifetime.clone(), lifetime_perm.clone().into())?;
        let lifetimes = self.extract_lifetime_variables_anonymise(ty)?;
        let lifetime_exprs: Vec<vir_low::Expression> = lifetimes
            .iter()
            .cloned()
            .map(|lifetime| lifetime.into())
            .collect();
        let restoration = {
            let restoration_snapshot = if value.is_mut {
                self.reference_target_final_snapshot(
                    result_type,
                    result_value.clone().into(),
                    position,
                )?
            } else {
                self.reference_target_current_snapshot(
                    result_type,
                    result_value.clone().into(),
                    position,
                )?
            };
            let validity =
                self.encode_snapshot_valid_call_for_type(restoration_snapshot.clone(), ty)?;
            expr! {
                wand(
                    (acc(DeadLifetimeToken(operand_lifetime))) --* (
                        (acc(OwnedNonAliased<ty>(
                            operand_place, operand_address, [restoration_snapshot]; lifetime_exprs
                        ))) &&
                        [validity] &&
                        // DeadLifetimeToken is duplicable and does not get consumed.
                        (acc(DeadLifetimeToken(operand_lifetime)))
                    )
                )
            }
        };
        let reference_target_address =
            self.reference_address(result_type, result_value.clone().into(), position)?;
        posts.push(expr! {
            operand_address == [reference_target_address]
        });
        // Note: We do not constraint the final snapshot, because it is fresh.
        let reference_target_current_snapshot = self.reference_target_current_snapshot(
            result_type,
            result_value.clone().into(),
            position,
        )?;
        posts.push(expr! {
            operand_value == [reference_target_current_snapshot]
        });
        pres.push(expr! {
            [vir_low::Expression::no_permission()] < lifetime_perm
        });
        pres.push(expr! {
            lifetime_perm < [vir_low::Expression::full_permission()]
        });
        pres.push(predicate);
        pres.push(lifetime_token.clone());
        posts.push(lifetime_token);
        posts.push(reference_predicate);
        posts.push(restoration);
        parameters.push(operand_place);
        parameters.push(operand_address);
        parameters.push(operand_value);
        parameters.push(operand_lifetime);
        parameters.push(lifetime_perm);
        let method = vir_low::MethodDecl::new(
            method_name,
            parameters,
            vec![result_value],
            pres,
            posts,
            None,
        );
        self.declare_method(method)?;
        self.builtin_methods_state
            .encoded_assign_methods
            .insert(method_name.to_string());
        Ok(())
    }
    fn encode_assign_operand(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: Option<&mut Vec<vir_low::Statement>>,
        operand_counter: u32,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<vir_low::VariableDecl> {
        use vir_low::macros::*;
        let value = self.encode_assign_operand_snapshot(operand_counter, operand)?;
        let ty = operand.expression.get_type();
        let lifetimes_ty = self.extract_lifetime_variables_anonymise(ty)?;
        let lifetimes: Vec<vir_low::Expression> =
            lifetimes_ty.iter().cloned().map(|x| x.into()).collect();
        match operand.kind {
            vir_mid::OperandKind::Copy | vir_mid::OperandKind::Move => {
                let place = self.encode_assign_operand_place(operand_counter)?;
                let root_address = self.encode_assign_operand_address(operand_counter)?;
                pres.push(
                    expr! { acc(OwnedNonAliased<ty>(place, root_address, value; lifetimes)) },
                );
                let post_predicate = if operand.kind == vir_mid::OperandKind::Copy {
                    expr! { acc(OwnedNonAliased<ty>(place, root_address, value)) }
                } else {
                    if let Some(pre_write_statements) = pre_write_statements {
                        // FIXME: This is wrong for the case when `ty` contains
                        // type variables. The correct encoding should
                        // recursively call copy_place<ty> or move_place<ty>,
                        // which then could be implemented as bodyless methods
                        // in case `ty` is a type variable.
                        self.encode_into_memory_block_method(ty)?;
                        pre_write_statements
                            .push(stmt! { call into_memory_block<ty>(place, root_address, value) });
                    }
                    let compute_address = ty!(Address);
                    let size_of = self.encode_type_size_expression(ty)?;
                    expr! { acc(MemoryBlock((ComputeAddress::compute_address(place, root_address)), [size_of])) }
                };
                posts.push(post_predicate);
                parameters.push(place);
                parameters.push(root_address);
            }
            vir_mid::OperandKind::Constant => {}
        }
        pres.push(self.encode_snapshot_valid_call_for_type(value.clone().into(), ty)?);
        parameters.push(value.clone());
        Ok(value)
    }
    fn encode_assign_operand_place(
        &mut self,
        operand_counter: u32,
    ) -> SpannedEncodingResult<vir_low::VariableDecl> {
        Ok(vir_low::VariableDecl::new(
            format!("operand{}_place", operand_counter),
            self.place_type()?,
        ))
    }
    fn encode_assign_operand_address(
        &mut self,
        operand_counter: u32,
    ) -> SpannedEncodingResult<vir_low::VariableDecl> {
        Ok(vir_low::VariableDecl::new(
            format!("operand{}_root_address", operand_counter),
            self.address_type()?,
        ))
    }
    fn encode_assign_operand_snapshot(
        &mut self,
        operand_counter: u32,
        operand: &vir_mid::Operand,
    ) -> SpannedEncodingResult<vir_low::VariableDecl> {
        Ok(vir_low::VariableDecl::new(
            format!("operand{}_value", operand_counter),
            operand.expression.get_type().to_snapshot(self)?,
        ))
    }
}

pub(in super::super) trait BuiltinMethodsInterface {
    fn encode_write_address_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_move_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_copy_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_write_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_havoc_owned_non_aliased_method(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()>;
    fn encode_memory_block_split_method(&mut self, ty: &vir_mid::Type)
        -> SpannedEncodingResult<()>;
    fn encode_memory_block_join_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_havoc_memory_block_method(&mut self, ty: &vir_mid::Type)
        -> SpannedEncodingResult<()>;
    fn encode_into_memory_block_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_assign_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        target: vir_mid::Expression,
        value: vir_mid::Rvalue,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    fn encode_consume_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        operand: vir_mid::Operand,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    fn encode_havoc_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        predicate: vir_mid::Predicate,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    fn encode_open_frac_bor_atomic_method(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft_tok_sep_take_method(&mut self, lft_count: usize) -> SpannedEncodingResult<()>;
    fn encode_lft_tok_sep_return_method(&mut self, lft_count: usize) -> SpannedEncodingResult<()>;
    fn encode_newlft_method(&mut self) -> SpannedEncodingResult<()>;
    fn encode_endlft_method(&mut self) -> SpannedEncodingResult<()>;
    fn encode_dead_inclusion_method(&mut self) -> SpannedEncodingResult<()>;
    fn encode_open_close_mut_ref_methods(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()>;
    fn encode_bor_shorten_method(
        &mut self,
        ty_with_lifetime: &vir_mid::Type,
    ) -> SpannedEncodingResult<()>;
}

impl<'p, 'v: 'p, 'tcx: 'v> BuiltinMethodsInterface for Lowerer<'p, 'v, 'tcx> {
    fn encode_write_address_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        let ty_without_lifetime = ty.clone().erase_lifetimes();
        if !self
            .builtin_methods_state
            .encoded_write_address_methods
            .contains(&ty_without_lifetime)
        {
            self.encode_snapshot_to_bytes_function(ty)?;
            self.encode_memory_block_predicate()?;
            use vir_low::macros::*;
            let parameters = self.extract_non_type_parameters_from_type(ty)?;
            let parameters_validity: vir_low::Expression = self
                .extract_non_type_parameters_from_type_validity(ty)?
                .into_iter()
                .conjoin();
            let size_of = self.encode_type_size_expression(&ty_without_lifetime)?;
            let to_bytes = ty! { Bytes };
            let method = method! {
                write_address<ty>(
                    address: Address,
                    value: {ty.to_snapshot(self)?},
                    *parameters
                ) returns ()
                    raw_code {
                        let bytes = self.encode_memory_block_bytes_expression(
                            address.clone().into(),
                            size_of.clone(),
                        )?;
                    }
                    requires ([parameters_validity]);
                    requires (acc(MemoryBlock((address), [size_of.clone()])));
                    ensures (acc(MemoryBlock((address), [size_of])));
                    ensures (([bytes]) == (Snap<ty>::to_bytes(value)));
            };
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_write_address_methods
                .insert(ty_without_lifetime.clone());
        }
        Ok(())
    }
    fn encode_move_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        // TODO: Remove code duplication with encode_copy_place_method and encode_write_place_method
        if !self
            .builtin_methods_state
            .encoded_move_place_methods
            .contains(ty)
        {
            self.encode_compute_address(ty)?;
            let span = self.encoder.get_type_definition_span_mid(ty)?;
            let position = self.register_error(
                span,
                ErrorCtxt::UnexpectedBuiltinMethod(BuiltinMethodKind::MovePlace),
            );
            use vir_low::macros::*;
            let size_of = self.encode_type_size_expression(ty)?;
            let compute_address = ty!(Address);
            let to_bytes = ty! { Bytes };
            let mut statements = Vec::new();
            var_decls! {
                    target_place: Place,
                    target_root_address: Address,
                    source_place: Place,
                    source_root_address: Address,
                    source_value: {ty.to_snapshot(self)?},
                lifetime: Lifetime
            };
            let source_address =
                expr! { ComputeAddress::compute_address(source_place, source_root_address) };
            let bytes =
                self.encode_memory_block_bytes_expression(source_address.clone(), size_of.clone())?;
            let target_address =
                expr! { ComputeAddress::compute_address(target_place, target_root_address) };
            self.mark_owned_non_aliased_as_unfolded(ty)?;
            let type_decl = self.encoder.get_type_decl_mid(ty)?;
            if let vir_mid::TypeDecl::Reference(_) = type_decl {
                statements.push(stmtp! { position =>
                    unfold OwnedNonAliased<ty>(source_place, source_root_address, source_value, lifetime)
                });
            } else {
                statements.push(stmtp! { position =>
                    unfold OwnedNonAliased<ty>(source_place, source_root_address, source_value)
                });
            }
            match &type_decl {
                vir_mid::TypeDecl::Bool
                | vir_mid::TypeDecl::Int(_)
                | vir_mid::TypeDecl::Float(_)
                | vir_mid::TypeDecl::Reference(_)
                | vir_mid::TypeDecl::Pointer(_) => {
                    self.encode_write_address_method(ty)?;
                    statements.push(stmtp! { position =>
                        // TODO: Replace with memcopy.
                        call write_address<ty>([target_address], source_value)
                    });
                }
                vir_mid::TypeDecl::TypeVar(_) => {
                    // TODO: fix this for Trusted
                    // move_place of a generic or trusted type has no body
                }
                vir_mid::TypeDecl::Tuple(decl) => {
                    if decl.arguments.is_empty() {
                        self.encode_write_address_method(ty)?;
                        statements.push(stmtp! { position =>
                            // TODO: Replace with memcopy.
                            call write_address<ty>([target_address], source_value)
                        });
                    } else {
                        unimplemented!()
                    }
                }
                vir_mid::TypeDecl::Trusted(decl) => {
                    // copied from struct
                    if decl.fields.is_empty() {
                        self.encode_write_address_method(ty)?;
                        statements.push(stmtp! { position =>
                            // TODO: Replace with memcopy.
                            call write_address<ty>([target_address], source_value)
                        });
                    } else {
                        // FIXME: support encode_memory_block_split_method for abstract types
                        // assert!(
                        //     !ty.is_trusted() && !ty.is_type_var(),
                        //     "Trying to split an abstract type."
                        // );
                        self.encode_memory_block_split_method(ty)?;
                        statements.push(stmtp! {
                            position =>
                            call memory_block_split<ty>(
                                [target_address], [vir_low::Expression::full_permission()]
                            )
                        });
                        for field in &decl.fields {
                            let source_field_place = self.encode_field_place(
                                ty,
                                field,
                                source_place.clone().into(),
                                position,
                            )?;
                            let target_field_place = self.encode_field_place(
                                ty,
                                field,
                                target_place.clone().into(),
                                position,
                            )?;
                            let source_field_value = self.obtain_struct_field_snapshot(
                                ty,
                                field,
                                source_value.clone().into(),
                                position,
                            )?;
                            let field_type = &field.ty;
                            self.encode_move_place_method(field_type)?;
                            statements.push(stmtp! { position =>
                                call move_place<field_type>(
                                    [target_field_place],
                                    target_root_address,
                                    [source_field_place],
                                    source_root_address,
                                    [source_field_value]
                                )
                            });
                        }
                        self.encode_memory_block_join_method(ty)?;
                        statements.push(stmtp! {
                            position =>
                            call memory_block_join<ty>(
                                [source_address], [vir_low::Expression::full_permission()]
                            )
                        });
                    }
                }
                vir_mid::TypeDecl::Struct(decl) => {
                    if decl.fields.is_empty() {
                        self.encode_write_address_method(ty)?;
                        statements.push(stmtp! { position =>
                            // TODO: Replace with memcopy.
                            call write_address<ty>([target_address], source_value)
                        });
                    } else {
                        assert!(
                            !ty.is_trusted() && !ty.is_type_var(),
                            "Trying to split an abstract type."
                        );
                        self.encode_memory_block_split_method(ty)?;
                        statements.push(stmtp! {
                            position =>
                            call memory_block_split<ty>(
                                [target_address], [vir_low::Expression::full_permission()]
                            )
                        });
                        for field in &decl.fields {
                            let source_field_place = self.encode_field_place(
                                ty,
                                field,
                                source_place.clone().into(),
                                position,
                            )?;
                            let target_field_place = self.encode_field_place(
                                ty,
                                field,
                                target_place.clone().into(),
                                position,
                            )?;
                            let source_field_value = self.obtain_struct_field_snapshot(
                                ty,
                                field,
                                source_value.clone().into(),
                                position,
                            )?;
                            let field_type = &field.ty;
                            self.encode_move_place_method(field_type)?;
                            statements.push(stmtp! { position =>
                                call move_place<field_type>(
                                    [target_field_place],
                                    target_root_address,
                                    [source_field_place],
                                    source_root_address,
                                    [source_field_value]
                                )
                            });
                        }
                        self.encode_memory_block_join_method(ty)?;
                        statements.push(stmtp! {
                            position =>
                            call memory_block_join<ty>(
                                [source_address], [vir_low::Expression::full_permission()]
                            )
                        });
                    }
                }
                vir_mid::TypeDecl::Enum(decl) => {
                    let discriminant_call =
                        self.obtain_enum_discriminant(source_value.clone().into(), ty, position)?;
                    self.encode_memory_block_split_method(ty)?;
                    statements.push(stmtp! {
                        position =>
                        call memory_block_split<ty>(
                            [target_address],
                            [vir_low::Expression::full_permission()],
                            [discriminant_call.clone()]
                        )
                    });
                    for (&discriminant_value, variant) in
                        decl.discriminant_values.iter().zip(&decl.variants)
                    {
                        let variant_index = variant.name.clone().into();
                        let condition = expr! {
                            [discriminant_call.clone()] == [discriminant_value.into()]
                        };
                        let source_variant_place = self.encode_enum_variant_place(
                            ty,
                            &variant_index,
                            source_place.clone().into(),
                            position,
                        )?;
                        let target_variant_place = self.encode_enum_variant_place(
                            ty,
                            &variant_index,
                            target_place.clone().into(),
                            position,
                        )?;
                        let source_variant_value = self.obtain_enum_variant_snapshot(
                            ty,
                            &variant_index,
                            source_value.clone().into(),
                            position,
                        )?;
                        let variant_ty = &ty.clone().variant(variant_index);
                        self.encode_move_place_method(variant_ty)?;
                        statements.push(stmtp! { position =>
                            call<condition> move_place<variant_ty>(
                                [target_variant_place],
                                target_root_address,
                                [source_variant_place],
                                source_root_address,
                                [source_variant_value]
                            )
                        });
                    }
                    let discriminant_field = decl.discriminant_field();
                    let target_discriminant_place = self.encode_field_place(
                        ty,
                        &discriminant_field,
                        target_place.clone().into(),
                        position,
                    )?;
                    let source_discriminant_place = self.encode_field_place(
                        ty,
                        &discriminant_field,
                        source_place.clone().into(),
                        position,
                    )?;
                    let discriminant_type = &decl.discriminant_type;
                    let source_discriminant_value = self.construct_constant_snapshot(
                        discriminant_type,
                        discriminant_call.clone(),
                        position,
                    )?;
                    self.encode_move_place_method(discriminant_type)?;
                    statements.push(stmtp! { position =>
                        call move_place<discriminant_type>(
                            [target_discriminant_place],
                            target_root_address,
                            [source_discriminant_place],
                            source_root_address,
                            [source_discriminant_value]
                        )
                    });
                    self.encode_memory_block_join_method(ty)?;
                    statements.push(stmtp! {
                        position =>
                        call memory_block_join<ty>(
                            [source_address],
                            [vir_low::Expression::full_permission()],
                            [discriminant_call]
                        )
                    });
                }
                vir_mid::TypeDecl::Union(_decl) => {
                    unimplemented!();
                }
                vir_mid::TypeDecl::Array(_) => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Sequence(_) => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Map(_) => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Never => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Closure(_) => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Unsupported(_) => {
                    unimplemented!()
                }
            }

            let arguments;
            let preconditions;
            let postconditions;
            if let vir_mid::TypeDecl::Reference(_) = type_decl {
                statements.push(stmtp! { position =>
                    fold OwnedNonAliased<ty>(target_place, target_root_address, source_value, lifetime)
                });
                arguments = vec![
                    target_place.clone(),
                    target_root_address.clone(),
                    source_place.clone(),
                    source_root_address.clone(),
                    source_value.clone(),
                    lifetime.clone(),
                ];
                let validity =
                    self.encode_snapshot_valid_call_for_type(source_value.clone().into(), ty)?;
                preconditions = vec![
                    expr! {(acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_root_address)), [size_of.clone()])))},
                    expr! {(acc(OwnedNonAliased<ty>(source_place, source_root_address, source_value, lifetime)))},
                ];
                postconditions = vec![
                    expr! {(acc(OwnedNonAliased<ty>(target_place, target_root_address, source_value, lifetime)))},
                    expr! {(acc(MemoryBlock((ComputeAddress::compute_address(source_place, source_root_address)), [size_of])))},
                    expr! {(([bytes]) == (Snap<ty>::to_bytes(source_value)))},
                    expr! {[validity]},
                ];
            } else {
                statements.push(stmtp! { position =>
                    fold OwnedNonAliased<ty>(target_place, target_root_address, source_value)
                });
                arguments = vec![
                    target_place.clone(),
                    target_root_address.clone(),
                    source_place.clone(),
                    source_root_address.clone(),
                    source_value.clone(),
                ];
                let validity =
                    self.encode_snapshot_valid_call_for_type(source_value.clone().into(), ty)?;
                preconditions = vec![
                    expr! {(acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_root_address)), [size_of.clone()])))},
                    expr! {(acc(OwnedNonAliased<ty>(source_place, source_root_address, source_value)))},
                ];
                postconditions = vec![
                    expr! {(acc(OwnedNonAliased<ty>(target_place, target_root_address, source_value)))},
                    expr! {(acc(MemoryBlock((ComputeAddress::compute_address(source_place, source_root_address)), [size_of])))},
                    expr! {(([bytes]) == (Snap<ty>::to_bytes(source_value)))},
                    expr! {[validity]},
                ];
            }

            // FIXME: Add method body for move_place for references
            let body = if ty.is_type_var() || ty.is_trusted() || ty.is_reference() {
                None
            } else {
                Some(statements)
            };
            let method = vir_low::MethodDecl::new(
                method_name! { move_place<ty> },
                arguments,
                Vec::new(),
                preconditions,
                postconditions,
                body,
            );
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_move_place_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_copy_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        // TODO: Remove code duplication with encode_move_place_method
        if !self
            .builtin_methods_state
            .encoded_copy_place_methods
            .contains(ty)
        {
            self.encode_compute_address(ty)?;
            self.encode_write_place_method(ty)?;
            let span = self.encoder.get_type_definition_span_mid(ty)?;
            let position = self.register_error(
                span,
                ErrorCtxt::UnexpectedBuiltinMethod(BuiltinMethodKind::MovePlace),
            );
            use vir_low::macros::*;
            let size_of = self.encode_type_size_expression(ty)?;
            let compute_address = ty!(Address);
            let mut statements = Vec::new();
            if ty.is_reference() {
                // TODO: fix copy_place for references
                let mut method = method! {
                    copy_place<ty>(
                        target_place: Place,
                        target_address: Address,
                        source_place: Place,
                        source_address: Address,
                        source_permission: Perm,
                        source_value: {ty.to_snapshot(self)?}
                    ) returns ()
                    requires ([vir_low::Expression::no_permission()] < source_permission);
                    requires (acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_address)), [size_of])));
                    requires (acc(OwnedNonAliased<ty>(source_place, source_address, source_value), source_permission));
                    ensures (acc(OwnedNonAliased<ty>(target_place, target_address, source_value)));
                    ensures (acc(OwnedNonAliased<ty>(source_place, source_address, source_value), source_permission));
                };
                method.body = None;
                self.declare_method(method)?;
                self.builtin_methods_state
                    .encoded_copy_place_methods
                    .insert(ty.clone());
                return Ok(());
            }
            let mut method = method! {
                copy_place<ty>(
                    target_place: Place,
                    target_address: Address,
                    source_place: Place,
                    source_address: Address,
                    source_permission: Perm,
                    source_value: {ty.to_snapshot(self)?}
                ) returns ()
                    raw_code {
                        self.encode_fully_unfold_owned_non_aliased(
                            &mut statements,
                            ty,
                            source_place.clone().into(),
                            &Into::<vir_low::Expression>::into(source_address.clone()),
                            Some(source_permission.clone().into()),
                            source_value.clone().into(),
                            position,
                        )?;
                        let address = expr! { ComputeAddress::compute_address(source_place, source_address) };
                        self.encode_fully_join_memory_block(
                            &mut statements,
                            ty,
                            address.clone(),
                            Some(source_permission.clone().into()),
                            position,
                        )?;
                        statements.push(stmtp! { position =>
                            call write_place<ty>(target_place, target_address, source_value)
                        });
                        let to_bytes = ty! { Bytes };
                        let memory_block_bytes =
                            self.encode_memory_block_bytes_expression(
                                address, size_of.clone()
                            )?;
                        statements.push(stmtp! { position =>
                            assert ([memory_block_bytes] == (Snap<ty>::to_bytes(source_value)))
                        });
                        self.encode_fully_split_memory_block(
                            &mut statements,
                            ty,
                            expr! { ComputeAddress::compute_address(source_place, source_address) },
                            Some(source_permission.clone().into()),
                            position,
                        )?;
                        self.encode_fully_fold_owned_non_aliased(
                            &mut statements,
                            ty,
                            source_place.clone().into(),
                            &Into::<vir_low::Expression>::into(source_address.clone()),
                            Some(source_permission.clone().into()),
                            source_value.clone().into(),
                            position,
                        )?;
                        let validity = self.encode_snapshot_valid_call_for_type(source_value.clone().into(), ty)?;
                    }
                    requires ([vir_low::Expression::no_permission()] < source_permission);
                    requires (acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_address)), [size_of])));
                    requires (acc(OwnedNonAliased<ty>(source_place, source_address, source_value), source_permission));
                    ensures (acc(OwnedNonAliased<ty>(target_place, target_address, source_value)));
                    ensures (acc(OwnedNonAliased<ty>(source_place, source_address, source_value), source_permission));
                    ensures ([validity]);
            };
            method.body = if ty.is_reference() {
                None
            } else {
                Some(statements)
            };
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_copy_place_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_write_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        if ty.is_type_var() {
            return Ok(());
        }
        // TODO: Remove code duplication with encode_copy_place_method and encode_write_place_method
        if !self
            .builtin_methods_state
            .encoded_write_place_methods
            .contains(ty)
        {
            self.encode_compute_address(ty)?;
            self.encode_write_address_method(ty)?;
            let span = self.encoder.get_type_definition_span_mid(ty)?;
            let position = self.register_error(
                span,
                ErrorCtxt::UnexpectedBuiltinMethod(BuiltinMethodKind::WritePlace),
            );
            use vir_low::macros::*;
            let compute_address = ty!(Address);
            let size_of = self.encode_type_size_expression(ty)?;
            let mut statements = Vec::new();
            var_decls! {
                place: Place,
                root_address: Address,
                value: {ty.to_snapshot(self)?}
            };
            let validity = self.encode_snapshot_valid_call_for_type(value.clone().into(), ty)?;
            let address = expr! { ComputeAddress::compute_address(place, root_address) };
            let type_decl = self.encoder.get_type_decl_mid(ty)?;
            self.mark_owned_non_aliased_as_unfolded(ty)?;
            let mut lifetime_args: Vec<vir_low::VariableDecl> = vec![];
            match &type_decl {
                vir_mid::TypeDecl::Bool
                | vir_mid::TypeDecl::Int(_)
                | vir_mid::TypeDecl::Float(_)
                | vir_mid::TypeDecl::Pointer(_)
                | vir_mid::TypeDecl::Sequence(_)
                | vir_mid::TypeDecl::Map(_) => {
                    self.encode_write_address_method(ty)?;
                    statements.push(stmtp! { position =>
                        call write_address<ty>([address.clone()], value)
                    });
                }
                vir_mid::TypeDecl::TypeVar(_) => {
                    unreachable!("Cannot write constants to variables of generic type.");
                }
                vir_mid::TypeDecl::Trusted(_) => {
                    unreachable!("Cannot write constants to variables of trusted type.");
                }
                vir_mid::TypeDecl::Tuple(decl) => {
                    if decl.arguments.is_empty() {
                        self.encode_write_address_method(ty)?;
                        statements.push(stmtp! { position =>
                            call write_address<ty>([address.clone()], value)
                        });
                    } else {
                        self.encode_memory_block_split_method(ty)?;
                        statements.push(stmtp! {
                            position =>
                            call memory_block_split<ty>(
                                [address.clone()], [vir_low::Expression::full_permission()]
                            )
                        });
                        for field in decl.iter_fields() {
                            let field_place = self.encode_field_place(
                                ty,
                                &field,
                                place.clone().into(),
                                position,
                            )?;
                            let field_value = self.obtain_struct_field_snapshot(
                                ty,
                                &field,
                                value.clone().into(),
                                position,
                            )?;
                            let field_type = &field.ty;
                            self.encode_write_place_method(field_type)?;
                            statements.push(stmtp! { position =>
                                call write_place<field_type>(
                                    [field_place], root_address, [field_value]
                                )
                            });
                        }
                    }
                }
                vir_mid::TypeDecl::Struct(decl) => {
                    if decl.fields.is_empty() {
                        self.encode_write_address_method(ty)?;
                        statements.push(stmtp! { position =>
                            call write_address<ty>([address.clone()], value)
                        });
                    } else {
                        self.encode_memory_block_split_method(ty)?;
                        statements.push(stmtp! {
                            position =>
                            call memory_block_split<ty>(
                                [address.clone()], [vir_low::Expression::full_permission()]
                            )
                        });
                        for field in &decl.fields {
                            let field_place =
                                self.encode_field_place(ty, field, place.clone().into(), position)?;
                            let field_value = self.obtain_struct_field_snapshot(
                                ty,
                                field,
                                value.clone().into(),
                                position,
                            )?;
                            if let vir_mid::Type::Reference(reference) = &field.ty {
                                let lifetime = self.encode_lifetime_const_into_variable(
                                    reference.lifetime.clone(),
                                )?;
                                lifetime_args.push(lifetime)
                            }
                            let field_type = &field.ty;
                            self.encode_write_place_method(field_type)?;
                            statements.push(stmtp! { position =>
                                call write_place<field_type>(
                                    [field_place], root_address, [field_value]
                                )
                            });
                        }
                    }
                }
                vir_mid::TypeDecl::Enum(decl) => {
                    let discriminant_call =
                        self.obtain_enum_discriminant(value.clone().into(), ty, position)?;
                    self.encode_memory_block_split_method(ty)?;
                    statements.push(stmtp! {
                        position =>
                        call memory_block_split<ty>(
                            [address.clone()],
                            [vir_low::Expression::full_permission()],
                            [discriminant_call.clone()]
                        )
                    });
                    for (&discriminant_value, variant) in
                        decl.discriminant_values.iter().zip(&decl.variants)
                    {
                        let variant_index = variant.name.clone().into();
                        let condition = expr! {
                            [discriminant_call.clone()] == [discriminant_value.into()]
                        };
                        let variant_place = self.encode_enum_variant_place(
                            ty,
                            &variant_index,
                            place.clone().into(),
                            position,
                        )?;
                        let variant_value = self.obtain_enum_variant_snapshot(
                            ty,
                            &variant_index,
                            value.clone().into(),
                            position,
                        )?;
                        let variant_ty = &ty.clone().variant(variant_index);
                        self.encode_write_place_method(variant_ty)?;
                        statements.push(stmtp! { position =>
                            call<condition> write_place<variant_ty>(
                                [variant_place], root_address, [variant_value]
                            )
                        });
                    }
                    let discriminant_field = decl.discriminant_field();
                    let discriminant_place = self.encode_field_place(
                        ty,
                        &discriminant_field,
                        place.clone().into(),
                        position,
                    )?;
                    let discriminant_type = &decl.discriminant_type;
                    let discriminant_value = self.construct_constant_snapshot(
                        discriminant_type,
                        discriminant_call,
                        position,
                    )?;
                    self.encode_write_place_method(discriminant_type)?;
                    statements.push(stmtp! { position =>
                        call write_place<discriminant_type>(
                            [discriminant_place], root_address, [discriminant_value]
                        )
                    });
                }
                vir_mid::TypeDecl::Union(decl) => {
                    let discriminant_call =
                        self.obtain_enum_discriminant(value.clone().into(), ty, position)?;
                    self.encode_memory_block_split_method(ty)?;
                    statements.push(stmtp! {
                        position =>
                        call memory_block_split<ty>(
                            [address.clone()],
                            [vir_low::Expression::full_permission()],
                            [discriminant_call.clone()]
                        )
                    });
                    for (&discriminant_value, variant) in
                        decl.discriminant_values.iter().zip(&decl.variants)
                    {
                        let variant_index = variant.name.clone().into();
                        let condition = expr! {
                            [discriminant_call.clone()] == [discriminant_value.into()]
                        };
                        let variant_place = self.encode_enum_variant_place(
                            ty,
                            &variant_index,
                            place.clone().into(),
                            position,
                        )?;
                        let variant_value = self.obtain_enum_variant_snapshot(
                            ty,
                            &variant_index,
                            value.clone().into(),
                            position,
                        )?;
                        let variant_ty = &ty.clone().variant(variant_index);
                        self.encode_write_place_method(variant_ty)?;
                        statements.push(stmtp! { position =>
                            call<condition> write_place<variant_ty>(
                                [variant_place], root_address, [variant_value]
                            )
                        });
                    }
                }
                vir_mid::TypeDecl::Array(_) => {
                    // FIXME: See the comment below.
                }
                vir_mid::TypeDecl::Reference(_) => {
                    // References should never be written through `write_place`.
                    return Ok(());
                }
                vir_mid::TypeDecl::Never => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Closure(_) => {
                    unimplemented!()
                }
                vir_mid::TypeDecl::Unsupported(_) => {
                    unimplemented!()
                }
            }
            statements.push(stmtp! { position =>
                fold OwnedNonAliased<ty>(place, root_address, value)
            });
            let body = if ty.is_array() {
                // TODO: We currently make write_place bodyless for arrays
                // because we would need builtin methods to support loops if we
                // wanted to implement the body.
                None
            } else if let vir_mid::TypeDecl::Struct(decl) = &type_decl {
                // TODO: We currently make write_place bodyless for structs with
                // reference-typed or generic fields
                let mut body = Some(statements);
                for field in decl.iter_fields() {
                    if field.ty.is_reference() || field.ty.is_type_var() {
                        body = None;
                    }
                }
                body
            } else {
                Some(statements)
            };
            let mut parameters = vec![place.clone(), root_address.clone(), value.clone()];
            parameters.extend(self.extract_non_type_parameters_from_type(ty)?);
            let mut args = self.extract_non_type_parameters_from_type_as_exprs(ty)?;
            parameters.extend(lifetime_args.clone());
            args.extend(
                lifetime_args
                    .iter()
                    .map(|x| x.clone().into())
                    .collect::<Vec<vir_low::Expression>>(),
            );
            let mut pres = self.extract_non_type_parameters_from_type_validity(ty)?;
            pres.push(expr! { (acc(MemoryBlock([address], [size_of]))) });
            pres.push(validity);
            let method = vir_low::MethodDecl::new(
                method_name! { write_place<ty> },
                parameters,
                Vec::new(),
                pres,
                vec![expr! { (acc(OwnedNonAliased<ty>(place, root_address, value; args))) }],
                body,
            );
            self.declare_method(method.set_default_position(position))?;
            self.builtin_methods_state
                .encoded_write_place_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_havoc_owned_non_aliased_method(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        let ty_without_lifetime = &ty.clone().erase_lifetimes();
        if !self
            .builtin_methods_state
            .encoded_owned_non_aliased_havoc_methods
            .contains(ty_without_lifetime)
        {
            use vir_low::macros::*;
            let method_name = self.encode_havoc_owned_non_aliased_method_name(ty)?;
            var_decls! {
                place: Place,
                root_address: Address,
                old_value: {ty.to_snapshot(self)?},
                fresh_value: {ty.to_snapshot(self)?}
            };
            let validity =
                self.encode_snapshot_valid_call_for_type(fresh_value.clone().into(), ty)?;
            let method = vir_low::MethodDecl::new(
                method_name,
                vec![place.clone(), root_address.clone(), old_value.clone()],
                vec![fresh_value.clone()],
                vec![expr! { (acc(OwnedNonAliased<ty>(place, root_address, old_value))) }],
                vec![
                    expr! { (acc(OwnedNonAliased<ty>(place, root_address, fresh_value))) },
                    validity,
                ],
                None,
            );
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_owned_non_aliased_havoc_methods
                .insert(ty_without_lifetime.clone());
        }
        Ok(())
    }
    fn encode_memory_block_split_method(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_memory_block_split_methods
            .contains(ty)
        {
            // FIXME: support encode_memory_block_split_method for abstract types
            // assert!(
            //     !ty.is_trusted() && !ty.is_type_var(),
            //     "Trying to split an abstract type."
            // );
            use vir_low::macros::*;
            let method = if ty.has_variants() {
                // TODO: remove code duplication with encode_memory_block_join_method
                let type_decl = self.encoder.get_type_decl_mid(ty)?;
                match type_decl {
                    vir_mid::TypeDecl::Enum(enum_decl) => {
                        var_decls!(address: Address, permission_amount: Perm, discriminant: Int);
                        let size_of = self.encode_type_size_expression(ty)?;
                        let whole_block =
                            expr!(acc(MemoryBlock(address, [size_of]), permission_amount));
                        let discriminant_field = enum_decl.discriminant_field();
                        let discriminant_size_of =
                            self.encode_type_size_expression(&enum_decl.discriminant_type)?;
                        let discriminant_address = self.encode_field_address(
                            ty,
                            &discriminant_field,
                            address.clone().into(),
                            Default::default(),
                        )?;
                        let mut postconditions = vec![
                            expr! { acc(MemoryBlock([discriminant_address], [discriminant_size_of]))},
                        ];
                        for (&discriminant_value, variant) in enum_decl
                            .discriminant_values
                            .iter()
                            .zip(&enum_decl.variants)
                        {
                            let variant_index = variant.name.clone().into();
                            let variant_address = self.encode_enum_variant_address(
                                ty,
                                &variant_index,
                                address.clone().into(),
                                Default::default(),
                            )?;
                            let variant_type = ty.clone().variant(variant_index);
                            let variant_size_of =
                                self.encode_type_size_expression(&variant_type)?;
                            postconditions.push(expr! {
                                (discriminant == [discriminant_value.into()]) ==>
                                (acc(MemoryBlock([variant_address], [variant_size_of])))
                            })
                        }
                        vir_low::MethodDecl::new(
                            method_name! { memory_block_split<ty> },
                            vec![address, permission_amount.clone(), discriminant],
                            Vec::new(),
                            vec![
                                expr! { [vir_low::Expression::no_permission()] < permission_amount },
                                whole_block,
                            ],
                            postconditions,
                            None,
                        )
                    }
                    vir_mid::TypeDecl::Union(enum_decl) => {
                        var_decls!(address: Address, permission_amount: Perm, discriminant: Int);
                        let size_of = self.encode_type_size_expression(ty)?;
                        let whole_block =
                            expr!(acc(MemoryBlock(address, [size_of]), permission_amount));
                        let mut postconditions = Vec::new();
                        for (&discriminant_value, variant) in enum_decl
                            .discriminant_values
                            .iter()
                            .zip(&enum_decl.variants)
                        {
                            let variant_index = variant.name.clone().into();
                            let variant_address = self.encode_enum_variant_address(
                                ty,
                                &variant_index,
                                address.clone().into(),
                                Default::default(),
                            )?;
                            let variant_type = ty.clone().variant(variant_index);
                            let variant_size_of =
                                self.encode_type_size_expression(&variant_type)?;
                            postconditions.push(expr! {
                                (discriminant == [discriminant_value.into()]) ==>
                                (acc(MemoryBlock([variant_address], [variant_size_of])))
                            })
                        }
                        vir_low::MethodDecl::new(
                            method_name! { memory_block_split<ty> },
                            vec![address, permission_amount.clone(), discriminant],
                            Vec::new(),
                            vec![
                                expr! { [vir_low::Expression::no_permission()] < permission_amount },
                                whole_block,
                            ],
                            postconditions,
                            None,
                        )
                    }
                    _ => {
                        unreachable!("only enums and unions has variants")
                    }
                }
            } else {
                let mut helper = SplitJoinHelper::new(false);
                helper.walk_type(ty, (), self)?;
                let to_bytes = ty! { Bytes };
                var_decls! { address: Address };
                let size_of = self.encode_type_size_expression(ty)?;
                let memory_block_bytes =
                    self.encode_memory_block_bytes_expression(address.into(), size_of)?;
                let bytes_quantifier = expr! {
                    forall(
                        snapshot: {ty.to_snapshot(self)?} ::
                        [ { (Snap<ty>::to_bytes(snapshot)) } ]
                        ((old([memory_block_bytes])) == (Snap<ty>::to_bytes(snapshot))) ==>
                        [ helper.field_to_bytes_equalities.into_iter().conjoin() ]
                    )
                };
                helper.postconditions.push(bytes_quantifier);
                vir_low::MethodDecl::new(
                    method_name! { memory_block_split<ty> },
                    vars! { address: Address, permission_amount: Perm },
                    Vec::new(),
                    helper.preconditions,
                    helper.postconditions,
                    None,
                )
            };
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_memory_block_split_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_memory_block_join_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_memory_block_join_methods
            .contains(ty)
        {
            use vir_low::macros::*;
            self.encode_snapshot_to_bytes_function(ty)?;
            let method = if ty.has_variants() {
                // TODO: remove code duplication with encode_memory_block_split_method
                let type_decl = self.encoder.get_type_decl_mid(ty)?;
                match type_decl {
                    vir_mid::TypeDecl::Enum(enum_decl) => {
                        var_decls!(address: Address, permission_amount: Perm, discriminant: Int);
                        let size_of = self.encode_type_size_expression(ty)?;
                        let whole_block = expr!(acc(MemoryBlock(address, [size_of.clone()])));
                        let discriminant_field = enum_decl.discriminant_field();
                        let discriminant_size_of =
                            self.encode_type_size_expression(&enum_decl.discriminant_type)?;
                        let discriminant_address = self.encode_field_address(
                            ty,
                            &discriminant_field,
                            address.clone().into(),
                            Default::default(),
                        )?;
                        let discriminant_expr: vir_low::Expression = discriminant.clone().into();
                        let discriminant_bounds = discriminant_expr
                            .generate_discriminant_bounds(&enum_decl.discriminant_bounds);
                        let mut preconditions = vec![
                            expr! { [vir_low::Expression::no_permission()] < permission_amount },
                            expr! {
                                acc(MemoryBlock(
                                    [discriminant_address.clone()],
                                    [discriminant_size_of.clone()]),
                                permission_amount)
                            },
                            discriminant_bounds,
                        ];
                        let to_bytes = ty! { Bytes };
                        let mut bytes_quantifier_conjuncts = Vec::new();
                        let memory_block_bytes = self.encode_memory_block_bytes_expression(
                            address.clone().into(),
                            size_of,
                        )?;
                        let snapshot: vir_low::Expression =
                            var! { snapshot: {ty.to_snapshot(self)?} }.into();
                        for (&discriminant_value, variant) in enum_decl
                            .discriminant_values
                            .iter()
                            .zip(&enum_decl.variants)
                        {
                            let variant_index = variant.name.clone().into();
                            let variant_address = self.encode_enum_variant_address(
                                ty,
                                &variant_index,
                                address.clone().into(),
                                Default::default(),
                            )?;

                            let variant_snapshot = self.obtain_enum_variant_snapshot(
                                ty,
                                &variant_index,
                                snapshot.clone(),
                                Default::default(),
                            )?;
                            let variant_type = &ty.clone().variant(variant_index);
                            let variant_size_of = self.encode_type_size_expression(variant_type)?;
                            preconditions.push(expr! {
                                (discriminant == [discriminant_value.into()]) ==>
                                (acc(MemoryBlock(
                                    [variant_address.clone()],
                                    [variant_size_of.clone()]),
                                permission_amount))
                            });
                            let memory_block_field_bytes = self
                                .encode_memory_block_bytes_expression(
                                    discriminant_address.clone(),
                                    discriminant_size_of.clone(),
                                )?;
                            let discriminant_type = &enum_decl.discriminant_type;
                            let discriminant_call = self.obtain_enum_discriminant(
                                snapshot.clone(),
                                ty,
                                Default::default(),
                            )?;
                            let discriminant_snapshot = self.construct_constant_snapshot(
                                discriminant_type,
                                discriminant_call,
                                Default::default(),
                            )?;
                            let memory_block_variant_bytes = self
                                .encode_memory_block_bytes_expression(
                                    variant_address,
                                    variant_size_of,
                                )?;
                            bytes_quantifier_conjuncts.push(expr! {
                                (discriminant == [discriminant_value.into()]) ==>
                                (
                                    (
                                        ((old([memory_block_field_bytes])) == (Snap<discriminant_type>::to_bytes([discriminant_snapshot]))) &&
                                        ((old([memory_block_variant_bytes])) == (Snap<variant_type>::to_bytes([variant_snapshot])))
                                    ) ==>
                                    ([memory_block_bytes.clone()] == (Snap<ty>::to_bytes([snapshot.clone()])))
                                )
                            });
                        }
                        let bytes_quantifier = expr! {
                            forall(
                                snapshot: {ty.to_snapshot(self)?} ::
                                [ { (Snap<ty>::to_bytes(snapshot)) } ]
                                [ bytes_quantifier_conjuncts.into_iter().conjoin() ]
                            )
                        };
                        vir_low::MethodDecl::new(
                            method_name! { memory_block_join<ty> },
                            vec![address, permission_amount, discriminant],
                            Vec::new(),
                            preconditions,
                            vec![whole_block, bytes_quantifier],
                            None,
                        )
                    }
                    vir_mid::TypeDecl::Union(enum_decl) => {
                        var_decls!(address: Address, permission_amount: Perm, discriminant: Int);
                        let size_of = self.encode_type_size_expression(ty)?;
                        let whole_block = expr!(acc(
                            MemoryBlock(address, [size_of.clone()]),
                            permission_amount
                        ));
                        let discriminant_expr: vir_low::Expression = discriminant.clone().into();
                        let discriminant_bounds = discriminant_expr
                            .generate_discriminant_bounds(&enum_decl.discriminant_bounds);
                        let mut preconditions = vec![
                            expr! { [vir_low::Expression::no_permission()] < permission_amount },
                            discriminant_bounds,
                        ];
                        let to_bytes = ty! { Bytes };
                        let mut bytes_quantifier_conjuncts = Vec::new();
                        let memory_block_bytes = self.encode_memory_block_bytes_expression(
                            address.clone().into(),
                            size_of,
                        )?;
                        let snapshot: vir_low::Expression =
                            var! { snapshot: {ty.to_snapshot(self)?} }.into();
                        for (&discriminant_value, variant) in enum_decl
                            .discriminant_values
                            .iter()
                            .zip(&enum_decl.variants)
                        {
                            let variant_index = variant.name.clone().into();
                            let variant_address = self.encode_enum_variant_address(
                                ty,
                                &variant_index,
                                address.clone().into(),
                                Default::default(),
                            )?;

                            let variant_snapshot = self.obtain_enum_variant_snapshot(
                                ty,
                                &variant_index,
                                snapshot.clone(),
                                Default::default(),
                            )?;
                            let variant_type = &ty.clone().variant(variant_index);
                            let variant_size_of = self.encode_type_size_expression(variant_type)?;
                            preconditions.push(expr! {
                                (discriminant == [discriminant_value.into()]) ==>
                                (acc(MemoryBlock(
                                    [variant_address.clone()],
                                    [variant_size_of.clone()]),
                                permission_amount))
                            });
                            let memory_block_variant_bytes = self
                                .encode_memory_block_bytes_expression(
                                    variant_address,
                                    variant_size_of,
                                )?;
                            bytes_quantifier_conjuncts.push(expr! {
                                (discriminant == [discriminant_value.into()]) ==>
                                (
                                    (
                                        ((old([memory_block_variant_bytes])) == (Snap<variant_type>::to_bytes([variant_snapshot])))
                                    ) ==>
                                    ([memory_block_bytes.clone()] == (Snap<ty>::to_bytes([snapshot.clone()])))
                                )
                            });
                        }
                        let bytes_quantifier = expr! {
                            forall(
                                snapshot: {ty.to_snapshot(self)?} ::
                                [ { (Snap<ty>::to_bytes(snapshot)) } ]
                                [ bytes_quantifier_conjuncts.into_iter().conjoin() ]
                            )
                        };
                        vir_low::MethodDecl::new(
                            method_name! { memory_block_join<ty> },
                            vec![address, permission_amount, discriminant],
                            Vec::new(),
                            preconditions,
                            vec![whole_block, bytes_quantifier],
                            None,
                        )
                    }
                    _ => {
                        unreachable!("only enums and unions has variants")
                    }
                }
            } else {
                let mut helper = SplitJoinHelper::new(true);
                helper.walk_type(ty, (), self)?;
                let to_bytes = ty! { Bytes };
                var_decls! { address: Address };
                let size_of = self.encode_type_size_expression(ty)?;
                let memory_block_bytes =
                    self.encode_memory_block_bytes_expression(address.into(), size_of)?;
                let bytes_quantifier = expr! {
                    forall(
                        snapshot: {ty.to_snapshot(self)?} ::
                        [ { (Snap<ty>::to_bytes(snapshot)) } ]
                        [ helper.field_to_bytes_equalities.into_iter().conjoin() ] ==>
                        ([memory_block_bytes] == (Snap<ty>::to_bytes(snapshot)))
                    )
                };
                helper.postconditions.push(bytes_quantifier);
                vir_low::MethodDecl::new(
                    method_name! { memory_block_join<ty> },
                    vars! { address: Address, permission_amount: Perm },
                    Vec::new(),
                    helper.preconditions,
                    helper.postconditions,
                    None,
                )
            };
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_memory_block_join_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_havoc_memory_block_method(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        let ty_without_lifetime = ty.clone().erase_lifetimes();
        if !self
            .builtin_methods_state
            .encoded_memory_block_havoc_methods
            .contains(&ty_without_lifetime)
        {
            use vir_low::macros::*;
            self.encode_snapshot_to_bytes_function(ty)?;
            let size_of = self.encode_type_size_expression(&ty_without_lifetime)?;
            let method = method! {
                havoc_memory_block<ty>(
                    address: Address
                ) returns ()
                    requires (acc(MemoryBlock((address), [size_of.clone()])));
                    ensures (acc(MemoryBlock((address), [size_of])));
            };
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_memory_block_havoc_methods
                .insert(ty_without_lifetime);
        }
        Ok(())
    }
    // FIXME: This method has to be inlined if the converted type has a resource
    // invariant in it. Otherwise, that resource would be leaked.
    fn encode_into_memory_block_method(
        &mut self,
        ty_with_lifetime: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        let ty: &mut vir_mid::Type = &mut ty_with_lifetime.clone().erase_lifetimes();
        if !self
            .builtin_methods_state
            .encoded_into_memory_block_methods
            .contains(ty)
        {
            self.builtin_methods_state
                .encoded_into_memory_block_methods
                .insert(ty.clone());
            use vir_low::macros::*;
            self.mark_owned_non_aliased_as_unfolded(ty)?;
            let size_of = self.encode_type_size_expression(ty)?;
            let compute_address = ty!(Address);
            let to_bytes = ty! { Bytes };
            let span = self.encoder.get_type_definition_span_mid(ty)?;
            let position = self.register_error(
                span,
                ErrorCtxt::UnexpectedBuiltinMethod(BuiltinMethodKind::IntoMemoryBlock),
            );
            let mut statements = Vec::new();
            let type_decl = self.encoder.get_type_decl_mid(ty)?;
            let mut parameters = self.extract_non_type_parameters_from_type(ty)?;
            let parameters_validity: vir_low::Expression = self
                .extract_non_type_parameters_from_type_validity(ty)?
                .into_iter()
                .conjoin();
            let mut arguments: Vec<vir_low::Expression> =
                self.extract_non_type_parameters_from_type_as_exprs(ty)?;
            let lifetimes: Vec<vir_low::Expression> = self
                .extract_lifetime_variables_anonymise(ty)?
                .iter()
                .cloned()
                .map(|x| x.into())
                .collect();
            arguments.extend(lifetimes.clone());
            let arguments2 = arguments.clone();
            let lifetime_params = self.extract_lifetime_variables_anonymise(ty)?;
            parameters.extend(lifetime_params);
            let mut method = method! {
                into_memory_block<ty>(
                    place: Place,
                    root_address: Address,
                    value: {ty.to_snapshot(self)?},
                    *parameters
                ) returns ()
                    raw_code {
                        let address = expr! {
                            ComputeAddress::compute_address(place, root_address)
                        };
                        let bytes = self.encode_memory_block_bytes_expression(
                            address.clone(), size_of.clone()
                        )?;
                        statements.push(stmtp! {
                            position =>
                            unfold OwnedNonAliased<ty>(place, root_address, value; arguments2)
                        });
                        let (memory_block_value, ref to_bytes_type) = if let vir_mid::Type::Reference(_) = ty {
                            (
                                self.reference_address_snapshot(
                                    ty,
                                    value.clone().into(),
                                    position,
                                )?,
                                self.reference_address_type(ty)?
                            )
                        } else {
                            (value.clone().into(), ty.clone())
                        };
                        match type_decl {
                            vir_mid::TypeDecl::Bool
                            | vir_mid::TypeDecl::Int(_)
                            | vir_mid::TypeDecl::Float(_)
                            | vir_mid::TypeDecl::Reference(_)
                            | vir_mid::TypeDecl::Pointer(_)
                            | vir_mid::TypeDecl::Sequence(_)
                            | vir_mid::TypeDecl::Map(_) => {
                                // Primitive type. Nothing to do.
                            }
                            vir_mid::TypeDecl::TypeVar(_) => unreachable!("cannot convert abstract type into a memory block: {}", ty),
                            vir_mid::TypeDecl::Tuple(decl) => {
                                // TODO: Remove code duplication.
                                for field in decl.iter_fields() {
                                    let field_place = self.encode_field_place(
                                        ty, &field, place.clone().into(), position
                                    )?;
                                    let field_value = self.obtain_struct_field_snapshot(
                                        ty, &field, value.clone().into(), position
                                    )?;
                                    self.encode_into_memory_block_method(&field.ty)?;
                                    let field_ty = &field.ty;
                                    statements.push(stmtp! {
                                        position =>
                                        call into_memory_block<field_ty>([field_place], root_address, [field_value])
                                    });
                                }
                                self.encode_memory_block_join_method(ty)?;
                                statements.push(stmtp! {
                                    position =>
                                    call memory_block_join<ty>(
                                        [address.clone()], [vir_low::Expression::full_permission()]
                                    )
                                });
                            },
                            vir_mid::TypeDecl::Trusted(decl) => {
                                // into_memory_block for trusted types is
                                // trusted and has no statements.
                                // TODO: check if this is still okay?
                                // copied from struct
                                // TODO: Remove code duplication.
                                for field in decl.iter_fields() {
                                    let field_place = self.encode_field_place(
                                        ty, &field, place.clone().into(), position
                                    )?;
                                    let field_value = self.obtain_struct_field_snapshot(
                                        ty, &field, value.clone().into(), position
                                    )?;
                                    self.encode_into_memory_block_method(&field.ty)?;
                                    let field_ty = &field.ty;
                                    statements.push(stmtp! {
                                        position =>
                                        call into_memory_block<field_ty>([field_place], root_address, [field_value])
                                    });
                                }
                                self.encode_memory_block_join_method(ty)?;
                                statements.push(stmtp! {
                                    position =>
                                    call memory_block_join<ty>(
                                        [address.clone()], [vir_low::Expression::full_permission()]
                                    )
                                });
                            },
                            vir_mid::TypeDecl::Struct(decl) => {
                                // TODO: Remove code duplication.
                                for field in decl.iter_fields() {
                                    let field_place = self.encode_field_place(
                                        ty, &field, place.clone().into(), position
                                    )?;
                                    let field_value = self.obtain_struct_field_snapshot(
                                        ty, &field, value.clone().into(), position
                                    )?;
                                    self.encode_into_memory_block_method(&field.ty)?;
                                    let field_ty = &field.ty;
                                    statements.push(stmtp! {
                                        position =>
                                        call into_memory_block<field_ty>([field_place], root_address, [field_value])
                                    });
                                }
                                self.encode_memory_block_join_method(ty)?;
                                statements.push(stmtp! {
                                    position =>
                                    call memory_block_join<ty>(
                                        [address.clone()], [vir_low::Expression::full_permission()]
                                    )
                                });
                            }
                            vir_mid::TypeDecl::Enum(decl) => {
                                let discriminant_call =
                                    self.obtain_enum_discriminant(value.clone().into(), ty, Default::default())?;
                                let discriminant_field = decl.discriminant_field();
                                let discriminant_place = self.encode_field_place(
                                    ty,
                                    &discriminant_field,
                                    place.clone().into(),
                                    position,
                                )?;
                                let discriminant_value =
                                    self.construct_constant_snapshot(&decl.discriminant_type, discriminant_call.clone(), position)?;
                                let discriminant_type = &decl.discriminant_type;
                                self.encode_into_memory_block_method(discriminant_type)?;
                                statements.push(stmtp! {
                                    position =>
                                    call into_memory_block<discriminant_type>([discriminant_place], root_address, [discriminant_value])
                                });
                                for (&discriminant, variant) in decl.discriminant_values.iter().zip(&decl.variants) {
                                    let variant_index = variant.name.clone().into();
                                    let variant_place = self.encode_enum_variant_place(
                                        ty, &variant_index, place.clone().into(), position,
                                    )?;
                                    let variant_value = self.obtain_enum_variant_snapshot(ty, &variant_index, value.clone().into(), position)?;
                                    let variant_ty = &ty.clone().variant(variant_index);
                                    self.encode_into_memory_block_method(variant_ty)?;
                                    let condition = expr! {
                                        [discriminant_call.clone()] == [discriminant.into()]
                                    };
                                    let condition1 = condition.clone();
                                    statements.push(stmtp! {
                                        position =>
                                        call<condition1> into_memory_block<variant_ty>([variant_place], root_address, [variant_value])
                                    });
                                    self.encode_memory_block_join_method(ty)?;
                                    statements.push(stmtp! {
                                        position =>
                                        call<condition> memory_block_join<ty>(
                                            [address.clone()],
                                            [vir_low::Expression::full_permission()],
                                            [discriminant.into()]
                                        )
                                    });
                                }
                            }
                            vir_mid::TypeDecl::Union(decl) => {
                                let discriminant_call =
                                    self.obtain_enum_discriminant(value.clone().into(), ty, Default::default())?;
                                for (&discriminant, variant) in decl.discriminant_values.iter().zip(&decl.variants) {
                                    let variant_index = variant.name.clone().into();
                                    let variant_place = self.encode_enum_variant_place(
                                        ty, &variant_index, place.clone().into(), position,
                                    )?;
                                    let variant_value = self.obtain_enum_variant_snapshot(ty, &variant_index, value.clone().into(), position)?;
                                    let variant_ty = &ty.clone().variant(variant_index);
                                    self.encode_into_memory_block_method(variant_ty)?;
                                    let condition = expr! {
                                        [discriminant_call.clone()] == [discriminant.into()]
                                    };
                                    let condition1 = condition.clone();
                                    statements.push(stmtp! {
                                        position =>
                                        call<condition1> into_memory_block<variant_ty>([variant_place], root_address, [variant_value])
                                    });
                                    self.encode_memory_block_join_method(ty)?;
                                    statements.push(stmtp! {
                                        position =>
                                        call<condition> memory_block_join<ty>(
                                            [address.clone()],
                                            [vir_low::Expression::full_permission()],
                                            [discriminant.into()]
                                        )
                                    });
                                }
                            }
                            vir_mid::TypeDecl::Array(_) => {
                            },
                            vir_mid::TypeDecl::Never => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Closure(_) => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Unsupported(_) => unimplemented!("ty: {}", ty),
                        };
                    }
                    requires ([parameters_validity]);
                    requires (acc(OwnedNonAliased<ty>(place, root_address, value; arguments)));
                    ensures (acc(MemoryBlock([address], [size_of])));
                    ensures (([bytes]) == (Snap<to_bytes_type>::to_bytes([memory_block_value])));
            };
            if !ty.is_trusted() && !ty.is_array() && !lifetimes.is_empty() {
                // FIXME: Encode the body for array. (Would require a loop to achieve this.)
                // FIXME: Encode the body for structs with lifetimes.
                method.body = None;
            }
            self.declare_method(method)?;
        }
        Ok(())
    }
    fn encode_assign_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        target: vir_mid::Expression,
        value: vir_mid::Rvalue,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        let method_name = self.encode_assign_method_name(target.get_type(), &value)?;
        self.encode_assign_method(&method_name, target.get_type(), &value)?;
        let target_place = self.encode_expression_as_place(&target)?;
        let target_address = self.extract_root_address(&target)?;
        let mut arguments = vec![target_place, target_address];
        self.encode_rvalue_arguments(&mut arguments, &value)?;
        arguments.extend(
            self.extract_non_type_arguments_from_type_excluding_lifetimes(target.get_type())?,
        );

        // let lifetimes = self.extract_lifetime_arguments_from_rvalue(&value)?;
        // let lifetime_exprs = lifetimes.iter().cloned().map(|x| x.into());
        // arguments.extend(lifetime_exprs);

        let target_value_type = target.get_type().to_snapshot(self)?;
        let result_value = self.create_new_temporary_variable(target_value_type)?;
        statements.push(vir_low::Statement::method_call(
            method_name,
            arguments,
            vec![result_value.clone().into()],
            position,
        ));
        self.encode_snapshot_update(statements, &target, result_value.clone().into(), position)?;
        if let vir_mid::Rvalue::Ref(value) = value {
            let snapshot = if value.is_mut {
                self.reference_target_final_snapshot(
                    target.get_type(),
                    result_value.into(),
                    position,
                )?
            } else {
                self.reference_target_current_snapshot(
                    target.get_type(),
                    result_value.into(),
                    position,
                )?
            };
            self.encode_snapshot_update(statements, &value.place, snapshot, position)?;
        }
        Ok(())
    }
    fn encode_consume_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        operand: vir_mid::Operand,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        let method_name = self.encode_consume_operand_method_name(&operand)?;
        self.encode_consume_operand_method(&method_name, &operand)?;
        let mut arguments = Vec::new();
        self.encode_operand_arguments(&mut arguments, &operand)?;
        let ty = operand.expression.get_type();
        if let vir_mid::ty::Type::Reference(reference) = ty {
            let lifetime = self.encode_lifetime_const_into_variable(reference.lifetime.clone())?;
            arguments.push(lifetime.into());
        }
        statements.push(vir_low::Statement::method_call(
            method_name,
            arguments,
            Vec::new(),
            position,
        ));
        Ok(())
    }
    fn encode_havoc_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        predicate: vir_mid::Predicate,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        match predicate {
            vir_mid::Predicate::OwnedNonAliased(predicate) => {
                let ty = predicate.place.get_type();
                self.mark_owned_non_aliased_as_unfolded(ty)?;
                self.encode_havoc_owned_non_aliased_method(ty)?;
                let place = self.encode_expression_as_place(&predicate.place)?;
                let address = self.extract_root_address(&predicate.place)?;
                let old_value = predicate.place.to_procedure_snapshot(self)?;
                let snapshot_type = ty.to_snapshot(self)?;
                let fresh_value = self.create_new_temporary_variable(snapshot_type)?;
                let method_name = self.encode_havoc_owned_non_aliased_method_name(ty)?;
                statements.push(vir_low::Statement::method_call(
                    method_name,
                    vec![place, address, old_value],
                    vec![fresh_value.clone().into()],
                    position,
                ));
                self.encode_snapshot_update(
                    statements,
                    &predicate.place,
                    fresh_value.into(),
                    position,
                )?;
            }
            vir_mid::Predicate::MemoryBlockStack(predicate) => {
                let ty = predicate.place.get_type();
                self.encode_havoc_memory_block_method(ty)?;
                let address = self.encode_expression_as_place_address(&predicate.place)?;
                use vir_low::macros::*;
                statements.push(stmtp! {
                    position =>
                    call havoc_memory_block<ty>([address])
                });
            }
            _ => unimplemented!(),
        }
        Ok(())
    }
    fn encode_open_frac_bor_atomic_method(
        &mut self,
        target_type: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_open_frac_bor_atomic_methods
            .contains(target_type)
        {
            self.builtin_methods_state
                .encoded_open_frac_bor_atomic_methods
                .insert(target_type.clone());
            self.encode_lifetime_token_predicate()?;
            use vir_low::macros::*;
            var_decls! {
                lifetime: Lifetime,
                lifetime_perm: Perm,
                owned_perm: Perm,
                place: Place,
                address: Address,
                current_snapshot: {target_type.to_snapshot(self)?}
            };
            let lifetime_access = expr! { acc(LifetimeToken(lifetime), lifetime_perm) };
            let frac_ref_access = expr! {
                acc(FracRef<target_type>(lifetime, place, address, current_snapshot))
            };
            let owned_access = expr! {
                acc(OwnedNonAliased<target_type>(place, address, current_snapshot), owned_perm)
            };
            let method = vir_low::MethodDecl::new(
                self.encode_open_frac_bor_atomic_method_name(target_type)?,
                vec![
                    lifetime,
                    lifetime_perm.clone(),
                    place,
                    address,
                    current_snapshot,
                ],
                vec![owned_perm.clone()],
                vec![
                    expr! {
                        [vir_low::Expression::no_permission()] < lifetime_perm
                    },
                    lifetime_access.clone(),
                    frac_ref_access.clone(),
                ],
                vec![
                    expr! {
                        owned_perm < [vir_low::Expression::full_permission()]
                    },
                    expr! {
                        [vir_low::Expression::no_permission()] < owned_perm
                    },
                    owned_access.clone(),
                    vir_low::Expression::magic_wand_no_pos(
                        owned_access,
                        expr! { [lifetime_access] && [frac_ref_access] },
                    ),
                ],
                None,
            );
            self.declare_method(method)?;
        }
        Ok(())
    }

    fn encode_lft_tok_sep_take_method(&mut self, lft_count: usize) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_lft_tok_sep_take_methods
            .contains(&lft_count)
        {
            self.builtin_methods_state
                .encoded_lft_tok_sep_take_methods
                .insert(lft_count);

            self.encode_lifetime_token_predicate()?;
            self.encode_lifetime_included()?;
            self.encode_lifetime_intersect(lft_count)?;
            self.encode_lifetime_included_intersect_axiom(lft_count)?;
            use vir_low::macros::*;

            let method_name = self.encode_lft_tok_sep_take_method_name(lft_count)?;
            var_decls!(rd_perm: Perm);

            // Parameters
            let mut parameters: Vec<vir_low::VariableDecl> =
                self.create_lifetime_var_decls(lft_count)?;

            // Preconditions
            let mut pres = vec![expr! {
                [vir_low::Expression::no_permission()] < rd_perm
            }];
            for lifetime in &parameters {
                pres.push(vir_low::Expression::predicate_access_predicate_no_pos(
                    stringify!(LifetimeToken).to_string(),
                    vec![lifetime.clone().into()],
                    rd_perm.clone().into(),
                ));
            }
            parameters.push(rd_perm.clone());

            // Postconditions
            var_decls!(lft: Lifetime);
            let lifetimes_expr: Vec<vir_low::Expression> =
                self.create_lifetime_expressions(lft_count)?;
            let posts = vec![
                // FIXME: use encode_lifetime_token from prusti-viper/src/encoder/middle/core_proof/lifetimes/interface.rs (to be merged)
                vir_low::Expression::predicate_access_predicate_no_pos(
                    stringify!(LifetimeToken).to_string(),
                    vec![lft.clone().into()],
                    rd_perm.into(),
                ),
                vir_low::Expression::binary_op_no_pos(
                    vir_low::BinaryOpKind::EqCmp,
                    expr! { lft },
                    vir_low::Expression::domain_function_call(
                        "Lifetime",
                        format!("intersect${}", lft_count),
                        lifetimes_expr,
                        ty!(Lifetime),
                    ),
                ),
            ];

            // Create Method
            let method =
                vir_low::MethodDecl::new(method_name, parameters, vec![lft], pres, posts, None);
            self.declare_method(method)?;
        }
        Ok(())
    }
    fn encode_lft_tok_sep_return_method(&mut self, lft_count: usize) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_lft_tok_sep_return_methods
            .contains(&lft_count)
        {
            self.builtin_methods_state
                .encoded_lft_tok_sep_return_methods
                .insert(lft_count);

            self.encode_lifetime_token_predicate()?;
            self.encode_lifetime_included()?;
            self.encode_lifetime_intersect(lft_count)?;
            self.encode_lifetime_included_intersect_axiom(lft_count)?;
            use vir_low::macros::*;

            let method_name = self.encode_lft_tok_sep_return_method_name(lft_count)?;
            var_decls!(lft: Lifetime); // target
            var_decls!(rd_perm: Perm);
            let lifetimes_var: Vec<vir_low::VariableDecl> =
                self.create_lifetime_var_decls(lft_count)?;
            let lifetimes_expr: Vec<vir_low::Expression> =
                self.create_lifetime_expressions(lft_count)?;

            // Parameters
            let mut parameters = vec![lft.clone()];
            parameters.append(lifetimes_var.clone().as_mut());
            parameters.push(rd_perm.clone());

            // Preconditions
            let pres = vec![
                expr! {
                    [vir_low::Expression::no_permission()] < rd_perm
                },
                vir_low::Expression::predicate_access_predicate_no_pos(
                    stringify!(LifetimeToken).to_string(),
                    vec![lft.clone().into()],
                    rd_perm.clone().into(),
                ),
                vir_low::Expression::binary_op_no_pos(
                    vir_low::BinaryOpKind::EqCmp,
                    expr! { lft },
                    vir_low::Expression::domain_function_call(
                        "Lifetime",
                        format!("intersect${}", lft_count),
                        lifetimes_expr,
                        ty!(Lifetime),
                    ),
                ),
            ];

            // Postconditions
            let posts: Vec<vir_low::Expression> = lifetimes_var
                .into_iter()
                .map(|lifetime| {
                    vir_low::Expression::predicate_access_predicate_no_pos(
                        stringify!(LifetimeToken).to_string(),
                        vec![lifetime.into()],
                        rd_perm.clone().into(),
                    )
                })
                .collect();

            // Create Method
            let method =
                vir_low::MethodDecl::new(method_name, parameters, vec![], pres, posts, None);
            self.declare_method(method)?;
        }
        Ok(())
    }
    fn encode_newlft_method(&mut self) -> SpannedEncodingResult<()> {
        if !self.builtin_methods_state.encoded_newlft_method {
            self.builtin_methods_state.encoded_newlft_method = true;
            self.encode_lifetime_token_predicate()?;
            use vir_low::macros::*;
            var_decls!(bw: Lifetime);
            let method = vir_low::MethodDecl::new(
                "newlft",
                Vec::new(),
                vec![bw.clone()],
                Vec::new(),
                vec![expr! { acc(LifetimeToken(bw)) }],
                None,
            );
            self.declare_method(method)?;
        }
        Ok(())
    }
    fn encode_dead_inclusion_method(&mut self) -> SpannedEncodingResult<()> {
        if !self.builtin_methods_state.encoded_dead_inclusion_method {
            self.builtin_methods_state.encoded_dead_inclusion_method = true;
            self.encode_lifetime_token_predicate()?;
            self.encode_lifetime_included()?;
            use vir_low::macros::*;
            var_decls! {
                lft_1: Lifetime,
                lft_2: Lifetime
            }
            let included = ty!(Bool);
            let pres = vec![
                expr! { acc(DeadLifetimeToken(lft_2))},
                expr! { Lifetime::included( [lft_1.clone().into()], [lft_2.clone().into()] ) },
            ];
            let posts = vec![
                expr! { acc(DeadLifetimeToken(lft_1))},
                expr! { acc(DeadLifetimeToken(lft_2))},
            ];
            let method = vir_low::MethodDecl::new(
                "dead_inclusion",
                vec![lft_1, lft_2],
                vec![],
                pres,
                posts,
                None,
            );
            self.declare_method(method)?;
        }
        Ok(())
    }
    fn encode_endlft_method(&mut self) -> SpannedEncodingResult<()> {
        if !self.builtin_methods_state.encoded_endlft_method {
            self.builtin_methods_state.encoded_endlft_method = true;
            self.encode_lifetime_token_predicate()?;
            use vir_low::macros::*;
            var_decls!(bw: Lifetime);
            let method = vir_low::MethodDecl::new(
                "endlft",
                vec![bw.clone()],
                Vec::new(),
                vec![expr! { acc(LifetimeToken(bw)) }],
                vec![expr! { acc(DeadLifetimeToken(bw)) }],
                None,
            );
            self.declare_method(method)?;
        }
        Ok(())
    }
    fn encode_open_close_mut_ref_methods(
        &mut self,
        target_type: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_open_close_mut_ref_methods
            .contains(target_type)
        {
            self.builtin_methods_state
                .encoded_open_close_mut_ref_methods
                .insert(target_type.clone());
            self.encode_lifetime_token_predicate()?;
            use vir_low::macros::*;

            var_decls! {
                lifetime: Lifetime,
                lifetime_perm: Perm,
                place: Place,
                address: Address,
                current_snapshot: {target_type.to_snapshot(self)?},
                final_snapshot: {target_type.to_snapshot(self)?}
            };
            let open_method = vir_low::MethodDecl::new(
                method_name! { open_mut_ref<target_type> },
                vec![
                    lifetime.clone(),
                    lifetime_perm.clone(),
                    place.clone(),
                    address.clone(),
                    current_snapshot.clone(),
                    final_snapshot.clone(),
                ],
                Vec::new(),
                vec![
                    expr! { [vir_low::Expression::no_permission()] < lifetime_perm },
                    expr! { acc(LifetimeToken(lifetime), lifetime_perm) },
                    expr! {
                        acc(UniqueRef<target_type>(
                            lifetime,
                            place,
                            address,
                            current_snapshot,
                            final_snapshot
                        ))
                    },
                ],
                vec![
                    expr! { acc(OwnedNonAliased<target_type>(
                        place,
                        address,
                        current_snapshot
                    ))},
                    // CloseMutRef predicate corresponds to the following
                    // viewshift:
                    // ```
                    // (acc(OwnedNonAliased<target_type>(
                    //     [deref_place],
                    //     [address_snapshot],
                    //     ?current_snapshot
                    // ))) --* (
                    //     (acc(LifetimeToken(lifetime), lifetime_perm)) &&
                    //     (acc(UniqueRef<target_type>(
                    //         lifetime,
                    //         [deref_place],
                    //         [address_snapshot],
                    //         current_snapshot,
                    //         [final_snapshot]
                    //     )))
                    // )
                    // ```
                    expr! { acc(CloseMutRef<target_type>(
                        lifetime,
                        lifetime_perm,
                        place,
                        address,
                        final_snapshot
                    ))},
                ],
                None,
            );
            self.declare_method(open_method)?;

            {
                let close_mut_ref_predicate = vir_low::PredicateDecl::new(
                    predicate_name! { CloseMutRef<target_type> },
                    vec![
                        lifetime.clone(),
                        lifetime_perm.clone(),
                        place.clone(),
                        address.clone(),
                        final_snapshot.clone(),
                    ],
                    None,
                );
                self.declare_predicate(close_mut_ref_predicate)?;
                // Apply the viewshift encoded in the `CloseMutRef` predicate.
                let close_method = vir_low::MethodDecl::new(
                    method_name! { close_mut_ref<target_type> },
                    vec![
                        lifetime.clone(),
                        lifetime_perm.clone(),
                        place.clone(),
                        address.clone(),
                        current_snapshot.clone(),
                        final_snapshot.clone(),
                    ],
                    Vec::new(),
                    vec![
                        expr! { [vir_low::Expression::no_permission()] < lifetime_perm },
                        expr! { acc(CloseMutRef<target_type>(
                            lifetime,
                            lifetime_perm,
                            place,
                            address,
                            final_snapshot
                        ))},
                        expr! { acc(OwnedNonAliased<target_type>(
                            place,
                            address,
                            current_snapshot
                        ))},
                    ],
                    vec![
                        expr! { acc(LifetimeToken(lifetime), lifetime_perm) },
                        expr! {
                            acc(UniqueRef<target_type>(
                                lifetime,
                                place,
                                address,
                                current_snapshot,
                                final_snapshot
                            ))
                        },
                    ],
                    None,
                );
                self.declare_method(close_method)?;
            }
        }
        Ok(())
    }
    fn encode_bor_shorten_method(
        &mut self,
        ty_with_lifetime: &vir_mid::Type,
    ) -> SpannedEncodingResult<()> {
        let ty: &mut vir_mid::Type = &mut ty_with_lifetime.clone().erase_lifetimes();
        if !self
            .builtin_methods_state
            .encoded_bor_shorten_methods
            .contains(ty)
        {
            self.builtin_methods_state
                .encoded_bor_shorten_methods
                .insert(ty.clone());
            use vir_low::macros::*;
            let type_decl = self.encoder.get_type_decl_mid(ty)?;
            let target_type = &type_decl.unwrap_reference().target_type;
            let included = ty!(Bool);
            var_decls! {
                lft: Lifetime,
                old_lft: Lifetime,
                lifetime_perm: Perm,
                place: Place,
                address: Address,
                current_snapshot: {target_type.to_snapshot(self)?},
                final_snapshot: {target_type.to_snapshot(self)?}
            }
            let pres = vec![
                expr! { [vir_low::Expression::no_permission()] < lifetime_perm },
                expr! { lifetime_perm < [vir_low::Expression::full_permission()] },
                expr! { Lifetime::included([lft.clone().into()], [old_lft.clone().into()])},
                expr! { acc(LifetimeToken(lft), lifetime_perm)},
                expr! {
                    acc(UniqueRef<target_type>(
                        old_lft,
                        place,
                        address,
                        current_snapshot,
                        final_snapshot
                    ))
                },
            ];
            let posts = vec![
                expr! { acc(LifetimeToken(lft), lifetime_perm)},
                expr! {
                    acc(UniqueRef<target_type>(
                        lft,
                        place,
                        address,
                        current_snapshot,
                        final_snapshot
                    ))
                },
            ];
            let method = vir_low::MethodDecl::new(
                method_name! { bor_shorten<ty> },
                vec![
                    lft,
                    old_lft,
                    lifetime_perm,
                    place,
                    address,
                    current_snapshot,
                    final_snapshot,
                ],
                vec![],
                pres,
                posts,
                None,
            );
            self.declare_method(method)?;
        }
        Ok(())
    }
}
