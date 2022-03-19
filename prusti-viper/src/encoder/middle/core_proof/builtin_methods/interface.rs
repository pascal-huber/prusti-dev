use super::helpers::ToAddressWriter;
use crate::encoder::{
    errors::{BuiltinMethodKind, ErrorCtxt, SpannedEncodingResult},
    high::types::HighTypeEncoderInterface,
    middle::core_proof::{
        addresses::AddressesInterface,
        builtin_methods::split_join::SplitJoinHelper,
        compute_address::ComputeAddressInterface,
        errors::ErrorsInterface,
        fold_unfold::FoldUnfoldInterface,
        into_low::IntoLowInterface,
        lowerer::{Lowerer, MethodsLowererInterface, VariablesLowererInterface},
        places::PlacesInterface,
        predicates_memory_block::PredicatesMemoryBlockInterface,
        snapshots::{IntoSnapshot, SnapshotsInterface},
        type_layouts::TypeLayoutsInterface,
        utils::type_decl_encoder::TypeDeclWalker,
    },
};
use rustc_hash::FxHashSet;
use vir_crate::{
    common::{expression::ExpressionIterator, identifier::WithIdentifier},
    low::{self as vir_low, operations::ToLow},
    middle::{self as vir_mid, operations::ty::Typed},
};

#[derive(Default)]
pub(in super::super) struct BuiltinMethodsState {
    encoded_write_place_methods: FxHashSet<vir_mid::Type>,
    encoded_move_place_methods: FxHashSet<vir_mid::Type>,
    encoded_copy_place_methods: FxHashSet<vir_mid::Type>,
    encoded_write_address_methods: FxHashSet<vir_mid::Type>,
    encoded_memory_block_split_methods: FxHashSet<vir_mid::Type>,
    encoded_memory_block_join_methods: FxHashSet<vir_mid::Type>,
    encoded_into_memory_block_methods: FxHashSet<vir_mid::Type>,
    encoded_assign_methods: FxHashSet<String>,
}

trait Private {
    #[allow(clippy::ptr_arg)] // Clippy false positive.
    fn encode_fully_split_write_address(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        ty: &vir_mid::Type,
        address: vir_low::Expression,
        value: vir_low::Expression,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    fn encode_assign_method_name(
        &self,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<String>;
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
    fn encode_assign_method(
        &mut self,
        method_name: &str,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_method_rvalue(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        post_write_statements: &mut Vec<vir_low::Statement>,
        value: &vir_mid::Rvalue,
        result_type: &vir_mid::Type,
        result_value: &vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
    #[allow(clippy::too_many_arguments)]
    fn encode_assign_operand(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        post_write_statements: &mut Vec<vir_low::Statement>,
        operand_counter: u32,
        operand: &vir_mid::Operand,
        position: vir_low::Position,
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
    fn encode_fully_split_write_address(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        ty: &vir_mid::Type,
        address: vir_low::Expression,
        value: vir_low::Expression,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        let mut writer = ToAddressWriter {
            statements,
            position,
        };
        writer.walk_type(ty, (address, value), self)
    }
    fn encode_assign_method_name(
        &self,
        ty: &vir_mid::Type,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<String> {
        Ok(format!(
            "assign${}${}",
            ty.get_identifier(),
            value.get_identifier()
        ))
    }
    fn encode_rvalue_arguments(
        &mut self,
        arguments: &mut Vec<vir_low::Expression>,
        value: &vir_mid::Rvalue,
    ) -> SpannedEncodingResult<()> {
        match value {
            vir_mid::Rvalue::UnaryOp(value) => {
                self.encode_operand_arguments(arguments, &value.argument)?;
            }
            vir_mid::Rvalue::BinaryOp(value) => {
                self.encode_operand_arguments(arguments, &value.left)?;
                self.encode_operand_arguments(arguments, &value.right)?;
            }
            vir_mid::Rvalue::Discriminant(value) => {
                self.encode_place_arguments(arguments, &value.place)?;
                // arguments.push(self.lower_expression_into_snapshot(&value.place)?);
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
                arguments.push(self.lower_expression_into_snapshot(&operand.expression)?);
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
        arguments.push(self.lower_expression_into_snapshot(expression)?);
        Ok(())
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
            var_decls! { result_value: {ty.create_snapshot(self)?} };
            let mut pres = vec![
                expr! { acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_address)), [size_of])) },
            ];
            let mut posts = vec![
                expr! { acc(OwnedNonAliased<ty>(target_place, target_address, result_value)) },
            ];
            let mut pre_write_statements = Vec::new();
            let mut post_write_statements = vec![stmtp! {
                position => call write_place<ty>(target_place, target_address, result_value)
            }];
            self.encode_assign_method_rvalue(
                &mut parameters,
                &mut pres,
                &mut posts,
                &mut pre_write_statements,
                &mut post_write_statements,
                value,
                ty,
                &result_value,
                position,
            )?;
            let mut statements = pre_write_statements;
            statements.extend(post_write_statements);
            let method = vir_low::MethodDecl::new(
                method_name,
                parameters,
                vec![result_value],
                pres,
                posts,
                Some(statements),
            );
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_assign_methods
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
        post_write_statements: &mut Vec<vir_low::Statement>,
        value: &vir_mid::Rvalue,
        result_type: &vir_mid::Type,
        result_value: &vir_low::VariableDecl,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()> {
        use vir_low::macros::*;
        let assigned_value = match value {
            vir_mid::Rvalue::UnaryOp(value) => {
                let operand_value = self.encode_assign_operand(
                    parameters,
                    pres,
                    posts,
                    pre_write_statements,
                    post_write_statements,
                    1,
                    &value.argument,
                    position,
                )?;
                self.encode_unary_op_call(
                    value.kind,
                    value.argument.expression.get_type(),
                    operand_value.into(),
                    position,
                )?
            }
            vir_mid::Rvalue::BinaryOp(_value) => {
                unimplemented!();
            }
            vir_mid::Rvalue::Discriminant(value) => {
                let ty = value.place.get_type();
                var_decls! {
                    operand_place: Place ,
                    operand_address: Address ,
                    operand_value: { ty.create_snapshot(self)? }
                };
                let predicate = expr! {
                    acc(OwnedNonAliased<ty>(operand_place, operand_address, operand_value))
                };
                pres.push(predicate.clone());
                posts.push(predicate);
                parameters.push(operand_place.clone());
                parameters.push(operand_address.clone());
                parameters.push(operand_value.clone());
                pres.push(self.encode_snapshot_validity_expression_for_type(
                    operand_value.clone().into(),
                    ty,
                )?);
                let discriminant_call =
                    self.encode_discriminant_call(operand_value.into(), ty, position)?;
                self.encode_constant_snapshot(result_type, discriminant_call, position)?
            }
        };
        posts.push(exprp! { position => result_value == [assigned_value.clone()]});
        pre_write_statements.push(vir_low::Statement::assign(
            result_value.clone(),
            assigned_value,
            position,
        ));
        Ok(())
    }
    fn encode_assign_operand(
        &mut self,
        parameters: &mut Vec<vir_low::VariableDecl>,
        pres: &mut Vec<vir_low::Expression>,
        posts: &mut Vec<vir_low::Expression>,
        pre_write_statements: &mut Vec<vir_low::Statement>,
        _post_write_statements: &mut Vec<vir_low::Statement>,
        operand_counter: u32,
        operand: &vir_mid::Operand,
        _position: vir_low::Position,
    ) -> SpannedEncodingResult<vir_low::VariableDecl> {
        use vir_low::macros::*;
        let value = self.encode_assign_operand_snapshot(operand_counter, operand)?;
        let ty = operand.expression.get_type();
        match operand.kind {
            vir_mid::OperandKind::Copy | vir_mid::OperandKind::Move => {
                let place = self.encode_assign_operand_place(operand_counter)?;
                let root_address = self.encode_assign_operand_address(operand_counter)?;
                pres.push(expr! { acc(OwnedNonAliased<ty>(place, root_address, value)) });
                let post_predicate = if operand.kind == vir_mid::OperandKind::Copy {
                    expr! { acc(OwnedNonAliased<ty>(place, root_address, value)) }
                } else {
                    pre_write_statements
                        .push(stmt! { call into_memory_block<ty>(place, root_address, value) });
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
        pres.push(self.encode_snapshot_validity_expression_for_type(value.clone().into(), ty)?);
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
            operand.expression.get_type().create_snapshot(self)?,
        ))
    }
}

pub(in super::super) trait BuiltinMethodsInterface {
    fn encode_write_address_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_move_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_copy_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_write_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_memory_block_split_method(&mut self, ty: &vir_mid::Type)
        -> SpannedEncodingResult<()>;
    fn encode_memory_block_join_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_into_memory_block_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()>;
    fn encode_assign_method_call(
        &mut self,
        statements: &mut Vec<vir_low::Statement>,
        target: vir_mid::Expression,
        value: vir_mid::Rvalue,
        position: vir_low::Position,
    ) -> SpannedEncodingResult<()>;
}

impl<'p, 'v: 'p, 'tcx: 'v> BuiltinMethodsInterface for Lowerer<'p, 'v, 'tcx> {
    fn encode_write_address_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_write_address_methods
            .contains(ty)
        {
            self.encode_snapshot_to_bytes_function(ty)?;
            self.encode_memory_block_predicate()?;
            use vir_low::macros::*;
            let size_of = self.encode_type_size_expression(ty)?;
            let to_bytes = ty! { Bytes };
            let method = method! {
                write_address<ty>(
                    address: Address,
                    value: {ty.create_snapshot(self)?}
                ) returns ()
                    raw_code {
                        let bytes = self.encode_memory_block_bytes_expression(
                            address.clone().into(),
                            size_of.clone(),
                        )?;
                    }
                    requires (acc(MemoryBlock((address), [size_of.clone()])));
                    ensures (acc(MemoryBlock((address), [size_of])));
                    ensures (([bytes]) == (Snap<ty>::to_bytes(value)));
            };
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_write_address_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_move_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        // TODO: Remove code duplication with encode_copy_place_method
        if !self
            .builtin_methods_state
            .encoded_move_place_methods
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
            let to_bytes = ty! { Bytes };
            let mut statements = Vec::new();
            let mut method = method! {
                move_place<ty>(
                    target_place: Place,
                    target_address: Address,
                    source_place: Place,
                    source_address: Address,
                    source_value: {ty.create_snapshot(self)?}
                ) returns ()
                    raw_code {
                        let compute_address_source = expr! { ComputeAddress::compute_address(source_place, source_address) };
                        let bytes = self.encode_memory_block_bytes_expression(compute_address_source, size_of.clone())?;
                        self.encode_fully_unfold_owned_non_aliased(
                            &mut statements,
                            ty,
                            source_place.clone().into(),
                            &Into::<vir_low::Expression>::into(source_address.clone()),
                            source_value.clone().into(),
                            position,
                        )?;
                        self.encode_fully_join_memory_block(
                            &mut statements,
                            ty,
                            expr! { ComputeAddress::compute_address(source_place, source_address) },
                            position,
                        )?;
                        statements.push(stmtp! { position => call write_place<ty>(target_place, target_address, source_value)});
                    }
                    requires (acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_address)), [size_of.clone()])));
                    requires (acc(OwnedNonAliased<ty>(source_place, source_address, source_value)));
                    ensures (acc(OwnedNonAliased<ty>(target_place, target_address, source_value)));
                    ensures (acc(MemoryBlock((ComputeAddress::compute_address(source_place, source_address)), [size_of])));
                    ensures (([bytes]) == (Snap<ty>::to_bytes(source_value)));
            };
            method.body = Some(statements);
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
            let mut method = method! {
                copy_place<ty>(
                    target_place: Place,
                    target_address: Address,
                    source_place: Place,
                    source_address: Address,
                    source_value: {ty.create_snapshot(self)?}
                ) returns ()
                    raw_code {
                        self.encode_fully_unfold_owned_non_aliased(
                            &mut statements,
                            ty,
                            source_place.clone().into(),
                            &Into::<vir_low::Expression>::into(source_address.clone()),
                            source_value.clone().into(),
                            position,
                        )?;
                        self.encode_fully_join_memory_block(
                            &mut statements,
                            ty,
                            expr! { ComputeAddress::compute_address(source_place, source_address) },
                            position,
                        )?;
                        statements.push(stmtp! { position => call write_place<ty>(target_place, target_address, source_value)});
                        self.encode_fully_fold_owned_non_aliased(
                            &mut statements,
                            ty,
                            source_place.clone().into(),
                            &Into::<vir_low::Expression>::into(source_address.clone()),
                            source_value.clone().into(),
                            position,
                        )?;
                    }
                    requires (acc(MemoryBlock((ComputeAddress::compute_address(target_place, target_address)), [size_of])));
                    requires (acc(OwnedNonAliased<ty>(source_place, source_address, source_value)));
                    ensures (acc(OwnedNonAliased<ty>(target_place, target_address, source_value)));
                    ensures (acc(OwnedNonAliased<ty>(source_place, source_address, source_value)));
            };
            method.body = Some(statements);
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_copy_place_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_write_place_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
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
            let mut method = method! {
                write_place<ty>(
                    place: Place,
                    address: Address,
                    value: {ty.create_snapshot(self)?}
                ) returns ()
                    raw_code {
                        let validity = self.encode_snapshot_validity_expression_for_type(value.clone().into(), ty)?;
                        let compute_address = expr! { ComputeAddress::compute_address(place, address) };
                        self.encode_fully_split_memory_block(
                            &mut statements,
                            ty,
                            compute_address.clone(),
                            position,
                        )?;
                        self.encode_fully_split_write_address(
                            &mut statements,
                            ty,
                            compute_address.clone(),
                            value.clone().into(),
                            position,
                        )?;
                        self.encode_fully_fold_owned_non_aliased(
                            &mut statements,
                            ty,
                            place.clone().into(),
                            &Into::<vir_low::Expression>::into(address.clone()),
                            value.clone().into(),
                            position,
                        )?;
                    }
                    requires (acc(MemoryBlock([compute_address], [size_of])));
                    requires ([validity]);
                    ensures (acc(OwnedNonAliased<ty>(place, address, value)));
            };
            method.body = Some(statements);
            self.declare_method(method.set_default_position(position))?;
            self.builtin_methods_state
                .encoded_write_place_methods
                .insert(ty.clone());
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
            let mut helper = SplitJoinHelper::new(false);
            helper.walk_type(ty, (), self)?;
            use vir_low::macros::*;
            let method = vir_low::MethodDecl::new(
                method_name! { memory_block_split<ty> },
                vars! { address: Address },
                Vec::new(),
                helper.preconditions,
                helper.postconditions,
                None,
            );
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
            let mut helper = SplitJoinHelper::new(true);
            helper.walk_type(ty, (), self)?;
            self.encode_snapshot_to_bytes_function(ty)?;
            use vir_low::macros::*;
            let to_bytes = ty! { Bytes };
            var_decls! { address: Address };
            let size_of = self.encode_type_size_expression(ty)?;
            let memory_block_bytes =
                self.encode_memory_block_bytes_expression(address.into(), size_of)?;
            let bytes_quantifier = expr! {
                forall(
                    snapshot: {ty.create_snapshot(self)?} ::
                    [ { (Snap<ty>::to_bytes(snapshot)) } ]
                    [ helper.field_to_bytes_equalities.into_iter().conjoin() ] ==>
                    ([memory_block_bytes] == (Snap<ty>::to_bytes(snapshot)))
                )
            };
            helper.postconditions.push(bytes_quantifier);
            let method = vir_low::MethodDecl::new(
                method_name! { memory_block_join<ty> },
                vars! { address: Address },
                Vec::new(),
                helper.preconditions,
                helper.postconditions,
                None,
            );
            self.declare_method(method)?;
            self.builtin_methods_state
                .encoded_memory_block_join_methods
                .insert(ty.clone());
        }
        Ok(())
    }
    fn encode_into_memory_block_method(&mut self, ty: &vir_mid::Type) -> SpannedEncodingResult<()> {
        if !self
            .builtin_methods_state
            .encoded_into_memory_block_methods
            .contains(ty)
        {
            self.builtin_methods_state
                .encoded_into_memory_block_methods
                .insert(ty.clone());
            use vir_low::macros::*;
            let size_of = self.encode_type_size_expression(ty)?;
            let compute_address = ty!(Address);
            let to_bytes = ty! { Bytes };
            let span = self.encoder.get_type_definition_span_mid(ty)?;
            let position = self.register_error(
                span,
                ErrorCtxt::UnexpectedBuiltinMethod(BuiltinMethodKind::IntoMemoryBlock),
            );
            let mut statements = Vec::new();
            let mut method = method! {
                into_memory_block<ty>(
                    place: Place,
                    root_address: Address,
                    value: {ty.create_snapshot(self)?}
                ) returns ()
                    raw_code {
                        let address = expr! {
                            ComputeAddress::compute_address(place, root_address)
                        };
                        let bytes = self.encode_memory_block_bytes_expression(
                            address.clone(), size_of.clone()
                        )?;
                        let type_decl = self.encoder.get_type_decl_mid(ty)?;
                        statements.push(stmtp! {
                            position =>
                            unfold OwnedNonAliased<ty>(place, root_address, value)
                        });
                        match type_decl {
                            vir_mid::TypeDecl::Bool
                            | vir_mid::TypeDecl::Int(_)
                            | vir_mid::TypeDecl::Float(_) => {
                                // Primitive type. Nothing to do.
                            }
                            vir_mid::TypeDecl::TypeVar(_) => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Tuple(decl) => {
                                // TODO: Remove code duplication.
                                for field in decl.iter_fields() {
                                    let field_place = self.encode_field_place(
                                        ty, &field, place.clone().into(), position
                                    )?;
                                    let field_value = self.encode_field_snapshot(
                                        ty, &field, value.clone().into(), position
                                    )?;
                                    self.encode_into_memory_block_method(&field.ty)?;
                                    let field_ty = &field.ty;
                                    statements.push(stmtp! {
                                        position =>
                                        call into_memory_block<field_ty>([field_place], root_address, [field_value])
                                    })
                                }
                                self.encode_memory_block_join_method(ty)?;
                                statements.push(stmtp! {
                                    position =>
                                    call memory_block_join<ty>([address.clone()])
                                });
                            },
                            vir_mid::TypeDecl::Struct(decl) => {
                                // TODO: Remove code duplication.
                                for field in decl.iter_fields() {
                                    let field_place = self.encode_field_place(
                                        ty, &field, place.clone().into(), position
                                    )?;
                                    let field_value = self.encode_field_snapshot(
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
                                    call memory_block_join<ty>([address.clone()])
                                });
                            }
                            vir_mid::TypeDecl::Enum(decl) => {
                                let discriminant_call =
                                    self.encode_discriminant_call(value.clone().into(), ty, Default::default())?;
                                for (discriminant, variant) in decl.discriminant_values.iter().zip(&decl.variants) {
                                    let variant_index = variant.name.clone().into();
                                    let variant_place = self.encode_enum_variant_place(
                                        ty, &variant_index, place.clone().into(), position,
                                    )?;
                                    let variant_value = self.encode_enum_variant_snapshot(ty, &variant_index, value.clone().into(), Default::default())?;
                                    let variant_ty = &ty.clone().variant(variant_index);
                                    self.encode_into_memory_block_method(variant_ty)?;
                                    let condition = expr! {
                                        [discriminant_call.clone()] == [discriminant.clone().to_low(self)?]
                                    };
                                    statements.push(stmtp! {
                                        position =>
                                        call<condition> into_memory_block<variant_ty>([variant_place], root_address, [variant_value])
                                    });
                                }
                                // let variant_name = place.get_variant_name(guiding_place);
                                // let variant = decl.variant(variant_name.as_ref()).unwrap();
                                // let ty = place.get_type().clone().variant(variant_name.clone());
                                // let variant_place = place.clone().variant_no_pos(variant_name.clone(), ty);
                                // expand_fields(&variant_place, variant.iter_fields())
                            }
                            vir_mid::TypeDecl::Array(_) => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Reference(_) => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Never => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Closure(_) => unimplemented!("ty: {}", ty),
                            vir_mid::TypeDecl::Unsupported(_) => unimplemented!("ty: {}", ty),
                        };
                        // self.encode_fully_unfold_owned_non_aliased(
                        //     &mut statements,
                        //     ty,
                        //     source_place.clone().into(),
                        //     &Into::<vir_low::Expression>::into(source_address.clone()),
                        //     source_value.clone().into(),
                        //     position,
                        // )?;
                        // self.encode_fully_join_memory_block(
                        //     &mut statements,
                        //     ty,
                        //     expr! { ComputeAddress::compute_address(source_place, source_address) },
                        //     position,
                        // )?;
                        // statements.push(stmtp! { position => call write_place<ty>(target_place, target_address, source_value)});
                    }
                    requires (acc(OwnedNonAliased<ty>(place, root_address, value)));
                    ensures (acc(MemoryBlock([address], [size_of])));
                    ensures (([bytes]) == (Snap<ty>::to_bytes(value)));
            };
            method.body = Some(statements);
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
        let target_value_type = target.get_type().create_snapshot(self)?;
        let result_value = self.create_new_temporary_variable(target_value_type)?;
        statements.push(vir_low::Statement::method_call(
            method_name,
            arguments,
            vec![result_value.clone().into()],
            position,
        ));
        self.encode_snapshot_update(statements, &target, result_value.into(), position)?;
        Ok(())
    }
}
