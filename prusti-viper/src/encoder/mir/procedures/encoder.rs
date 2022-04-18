use super::MirProcedureEncoderInterface;
use crate::encoder::{
    borrows::ProcedureContractMirDef,
    errors::{ErrorCtxt, SpannedEncodingError, SpannedEncodingResult, WithSpan},
    mir::{
        casts::CastsEncoderInterface,
        constants::ConstantsEncoderInterface,
        errors::ErrorInterface,
        generics::MirGenericsEncoderInterface,
        panics::MirPanicsEncoderInterface,
        places::PlacesEncoderInterface,
        predicates::MirPredicateEncoderInterface,
        pure::{PureFunctionEncoderInterface, SpecificationEncoderInterface},
        spans::SpanInterface,
        specifications::SpecificationsInterface,
        type_layouts::MirTypeLayoutsEncoderInterface,
    },
    mir_encoder::PRECONDITION_LABEL,
    Encoder,
};
use log::debug;
use prusti_common::config;
use prusti_interface::environment::{
    mir_dump::{graphviz::ToText, lifetimes::Lifetimes},
    Procedure,
};
use rustc_data_structures::graph::WithStartNode;
use rustc_hir::def_id::DefId;
use rustc_middle::{mir, ty, ty::subst::SubstsRef};
use rustc_span::Span;
use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet},
};
use vir_crate::{
    common::{
        expression::{BinaryOperationHelpers, UnaryOperationHelpers},
        position::Positioned,
    },
    high::{
        self as vir_high,
        builders::procedure::{
            BasicBlockBuilder, ProcedureBuilder, SuccessorBuilder, SuccessorExitKind,
        },
        operations::ty::Typed,
    },
};

pub(super) fn encode_procedure<'v, 'tcx: 'v>(
    encoder: &mut Encoder<'v, 'tcx>,
    def_id: DefId,
) -> SpannedEncodingResult<vir_high::ProcedureDecl> {
    let procedure = Procedure::new(encoder.env(), def_id);
    let lifetimes = if let Some(facts) = encoder
        .env()
        .try_get_local_mir_borrowck_facts(def_id.expect_local())
    {
        Lifetimes::new(facts)
    } else {
        return Err(SpannedEncodingError::internal(
            format!("failed to obtain borrow information for {:?}", def_id),
            procedure.get_span(),
        ));
    };
    let mir = procedure.get_mir();
    let locals_without_explicit_allocation: BTreeSet<_> = mir.vars_and_temps_iter().collect();
    let mut procedure_encoder = ProcedureEncoder {
        encoder,
        def_id,
        _procedure: &procedure,
        mir,
        lifetimes,
        check_panics: config::check_panics(),
        locals_without_explicit_allocation,
        fresh_id_generator: 0,
    };
    procedure_encoder.encode()
}

struct ProcedureEncoder<'p, 'v: 'p, 'tcx: 'v> {
    encoder: &'p mut Encoder<'v, 'tcx>,
    def_id: DefId,
    _procedure: &'p Procedure<'tcx>,
    mir: &'p mir::Body<'tcx>,
    lifetimes: Lifetimes,
    check_panics: bool,
    /// Locals that are not explicitly allocated or deallocated with
    /// `StorageLive`/`StorageDead`. Such locals are assumed to be alive through
    /// the entire body of the function.
    locals_without_explicit_allocation: BTreeSet<mir::Local>,
    fresh_id_generator: usize,
}

impl<'p, 'v: 'p, 'tcx: 'v> ProcedureEncoder<'p, 'v, 'tcx> {
    fn encode(&mut self) -> SpannedEncodingResult<vir_high::ProcedureDecl> {
        let name = self.encoder.encode_item_name(self.def_id);
        let (allocate_parameters, deallocate_parameters) = self.encode_parameters()?;
        let (allocate_returns, deallocate_returns) = self.encode_returns()?;
        let (assume_preconditions, assert_postconditions) =
            self.encode_functional_specifications()?;
        let mut procedure_builder = ProcedureBuilder::new(
            name.clone(),
            allocate_parameters.clone(),
            allocate_returns.clone(),
            assume_preconditions.clone(),
            deallocate_parameters.clone(),
            deallocate_returns.clone(),
            assert_postconditions.clone(),
        );
        // To "merge" blocks (if-else), we require info about the lifetimes of the "other" block
        // lets collect the info here:
        // - original lifetimes created and not ended in bb
        //   example lookup: original_lifetimes[bb1] = ["bw0"]
        let mut introduced_original_lifetimes: BTreeMap<mir::BasicBlock, BTreeSet<String>> =
            BTreeMap::new();
        // - derived lifetimes created and not ended in bb
        //   example lookup: derived_lifetimes[bb1] = [("lft6" -> ["bw0"])]
        let mut introduced_derived_lifetimes: BTreeMap<
            mir::BasicBlock,
            BTreeMap<String, BTreeSet<String>>,
        > = BTreeMap::new();

        // TODO: thats still not quite all of them
        let mut all_lifetimes: BTreeSet<String> = BTreeSet::new();

        self.extract_lifetimes(
            &mut procedure_builder,
            &mut introduced_original_lifetimes,
            &mut introduced_derived_lifetimes,
            &mut all_lifetimes,
        )?;
        // mayday mayday mayday, we need a new procedure builder, the old one is broken
        procedure_builder = ProcedureBuilder::new(
            name,
            allocate_parameters,
            allocate_returns,
            assume_preconditions,
            deallocate_parameters,
            deallocate_returns,
            assert_postconditions,
        );
        // println!("lifetimes created:");
        // dbg!(&introduced_original_lifetimes);
        // println!("lifetimes derived:");
        // dbg!(&introduced_derived_lifetimes);

        let rd_perm: u32 = all_lifetimes.len().try_into().unwrap();
        println!("##### rd_perm:");
        dbg!(&rd_perm);
        self.encode_body(
            &mut procedure_builder,
            &introduced_original_lifetimes,
            &introduced_derived_lifetimes,
            rd_perm,
        )?;
        self.encode_implicit_allocations(&mut procedure_builder)?;
        Ok(procedure_builder.build())
    }

    fn encode_local(
        &mut self,
        mir_local: mir::Local,
    ) -> SpannedEncodingResult<vir_high::expression::Local> {
        let span = self.encoder.get_local_span(self.mir, mir_local)?;
        let position = self
            .encoder
            .register_error(span, ErrorCtxt::Unexpected, self.def_id);
        let variable = self.encoder.encode_local_high(self.mir, mir_local)?;
        Ok(vir_high::expression::Local::new_with_pos(
            variable, position,
        ))
    }

    fn encode_parameters(
        &mut self,
    ) -> SpannedEncodingResult<(Vec<vir_high::Statement>, Vec<vir_high::Statement>)> {
        let mut allocation = vec![vir_high::Statement::comment(
            "Allocate the parameters.".to_string(),
        )];
        let mut deallocation = vec![vir_high::Statement::comment(
            "Deallocate the parameters.".to_string(),
        )];
        for mir_arg in self.mir.args_iter() {
            let parameter = self.encode_local(mir_arg)?;
            let alloc_statement = vir_high::Statement::inhale_no_pos(
                vir_high::Predicate::owned_non_aliased_no_pos(parameter.clone().into()),
            );
            allocation.push(self.encoder.set_statement_error_ctxt_from_position(
                alloc_statement,
                parameter.position,
                ErrorCtxt::UnexpectedStorageLive,
            )?);
            let mir_type = self.encoder.get_local_type(self.mir, mir_arg)?;
            let size = self.encoder.encode_type_size_expression(mir_type)?;
            let dealloc_statement = vir_high::Statement::exhale_no_pos(
                vir_high::Predicate::memory_block_stack_no_pos(parameter.clone().into(), size),
            );
            deallocation.push(self.encoder.set_statement_error_ctxt_from_position(
                dealloc_statement,
                parameter.position,
                ErrorCtxt::UnexpectedStorageDead,
            )?);
        }
        Ok((allocation, deallocation))
    }

    fn encode_returns(
        &mut self,
    ) -> SpannedEncodingResult<(Vec<vir_high::Statement>, Vec<vir_high::Statement>)> {
        let return_local = self.encode_local(mir::RETURN_PLACE)?;
        let mir_type = self.encoder.get_local_type(self.mir, mir::RETURN_PLACE)?;
        let size = self.encoder.encode_type_size_expression(mir_type)?;
        let alloc_statement = self.encoder.set_statement_error_ctxt_from_position(
            vir_high::Statement::inhale_no_pos(vir_high::Predicate::memory_block_stack_no_pos(
                return_local.clone().into(),
                size,
            )),
            return_local.position,
            ErrorCtxt::UnexpectedStorageLive,
        )?;
        let dealloc_statement = self.encoder.set_statement_error_ctxt_from_position(
            vir_high::Statement::exhale_no_pos(vir_high::Predicate::owned_non_aliased_no_pos(
                return_local.clone().into(),
            )),
            return_local.position,
            ErrorCtxt::UnexpectedStorageDead,
        )?;
        Ok((
            vec![
                vir_high::Statement::comment("Allocate the return place.".to_string()),
                alloc_statement,
            ],
            vec![
                vir_high::Statement::comment("Deallocate the return place.".to_string()),
                dealloc_statement,
            ],
        ))
    }

    fn encode_precondition_expressions(
        &mut self,
        procedure_contract: &ProcedureContractMirDef<'tcx>,
        call_substs: SubstsRef<'tcx>,
        arguments: &[vir_high::Expression],
    ) -> SpannedEncodingResult<Vec<vir_high::Expression>> {
        let mut preconditions = Vec::new();
        for (assertion, assertion_substs) in
            procedure_contract.functional_precondition(self.encoder.env(), call_substs)
        {
            let expression = self.encoder.encode_assertion_high(
                &assertion,
                None,
                arguments,
                None,
                self.def_id,
                assertion_substs,
            )?;
            preconditions.push(expression);
        }
        Ok(preconditions)
    }

    fn encode_postcondition_expressions(
        &mut self,
        procedure_contract: &ProcedureContractMirDef<'tcx>,
        call_substs: SubstsRef<'tcx>,
        arguments: Vec<vir_high::Expression>,
        result: &vir_high::Expression,
        precondition_label: &str,
    ) -> SpannedEncodingResult<Vec<vir_high::Expression>> {
        let mut postconditions = Vec::new();
        let arguments_in_old: Vec<_> = arguments
            .into_iter()
            .map(|argument| {
                let position = argument.position();
                vir_high::Expression::labelled_old(
                    precondition_label.to_string(),
                    argument,
                    position,
                )
            })
            .collect();
        for (assertion, assertion_substs) in
            procedure_contract.functional_postcondition(self.encoder.env(), call_substs)
        {
            let expression = self.encoder.encode_assertion_high(
                &assertion,
                Some(precondition_label),
                &arguments_in_old,
                Some(result),
                self.def_id,
                assertion_substs,
            )?;
            postconditions.push(expression);
        }
        Ok(postconditions)
    }

    fn encode_functional_specifications(
        &mut self,
    ) -> SpannedEncodingResult<(Vec<vir_high::Statement>, Vec<vir_high::Statement>)> {
        let mir_span = self.mir.span;
        let substs = self.encoder.env().identity_substs(self.def_id);
        // Retrieve the contract
        let procedure_contract = self
            .encoder
            .get_mir_procedure_contract_for_def(self.def_id, substs)
            .with_span(mir_span)?;
        let mut preconditions = vec![vir_high::Statement::comment(
            "Assume functional preconditions.".to_string(),
        )];
        let mut arguments: Vec<vir_high::Expression> = Vec::new();
        for local in self.mir.args_iter() {
            arguments.push(self.encode_local(local)?.into());
        }
        for expression in
            self.encode_precondition_expressions(&procedure_contract, substs, &arguments)?
        {
            let assume_statement = self.encoder.set_statement_error_ctxt(
                vir_high::Statement::assume_no_pos(expression),
                mir_span,
                ErrorCtxt::UnexpectedAssumeMethodPrecondition,
                self.def_id,
            )?;
            preconditions.push(assume_statement);
        }
        let old_label = self.encoder.set_statement_error_ctxt(
            vir_high::Statement::old_label_no_pos(PRECONDITION_LABEL.to_string()),
            mir_span,
            ErrorCtxt::UnexpectedAssumeMethodPrecondition,
            self.def_id,
        )?;
        preconditions.push(old_label);
        let mut postconditions = vec![vir_high::Statement::comment(
            "Assert functional postconditions.".to_string(),
        )];
        let result: vir_high::Expression = self.encode_local(mir::RETURN_PLACE)?.into();
        for expression in self.encode_postcondition_expressions(
            &procedure_contract,
            substs,
            arguments,
            &result,
            PRECONDITION_LABEL,
        )? {
            let assert_statement = self.encoder.set_statement_error_ctxt(
                vir_high::Statement::assert_no_pos(expression),
                mir_span,
                ErrorCtxt::AssertMethodPostcondition,
                self.def_id,
            )?;
            postconditions.push(assert_statement);
        }
        Ok((preconditions, postconditions))
    }

    fn encode_implicit_allocations(
        &mut self,
        procedure_builder: &mut ProcedureBuilder,
    ) -> SpannedEncodingResult<()> {
        procedure_builder.add_alloc_statement(vir_high::Statement::comment(
            "Allocate implicitly allocated statements.".to_string(),
        ));
        for local in std::mem::take(&mut self.locals_without_explicit_allocation) {
            let encoded_local = self.encode_local(local)?;
            let mir_type = self.encoder.get_local_type(self.mir, local)?;
            let size = self.encoder.encode_type_size_expression(mir_type)?;
            let predicate =
                vir_high::Predicate::memory_block_stack_no_pos(encoded_local.clone().into(), size);
            procedure_builder.add_alloc_statement(
                self.encoder.set_statement_error_ctxt_from_position(
                    vir_high::Statement::inhale_no_pos(predicate.clone()),
                    encoded_local.position,
                    ErrorCtxt::UnexpectedStorageLive,
                )?,
            );
            procedure_builder.add_dealloc_statement(
                self.encoder.set_statement_error_ctxt_from_position(
                    vir_high::Statement::exhale_no_pos(predicate.clone()),
                    encoded_local.position,
                    ErrorCtxt::UnexpectedStorageLive,
                )?,
            );
        }
        Ok(())
    }

    fn extract_lifetimes(
        &mut self,
        procedure_builder: &mut ProcedureBuilder,
        original_lifetimes: &mut BTreeMap<mir::BasicBlock, BTreeSet<String>>,
        derived_lifetimes: &mut BTreeMap<mir::BasicBlock, BTreeMap<String, BTreeSet<String>>>,
        all_lifetimes: &mut BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        let entry_label = vir_high::BasicBlockId::new("label_entry".to_string());
        let mut block_builder = procedure_builder.create_basic_block_builder(entry_label.clone());
        if self.mir.basic_blocks().is_empty() {
            block_builder.set_successor_exit(SuccessorExitKind::Return);
        } else {
            block_builder.set_successor_jump(vir_high::Successor::Goto(
                self.encode_basic_block_label(self.mir.start_node()),
            ));
        }
        block_builder.build();
        procedure_builder.set_entry(entry_label);
        for bb in self.mir.basic_blocks().indices() {
            self.extract_lifetimes_basic_block(
                bb,
                original_lifetimes,
                derived_lifetimes,
                all_lifetimes,
            )?;
        }
        Ok(())
    }

    fn encode_body(
        &mut self,
        procedure_builder: &mut ProcedureBuilder,
        introduced_original_lifetimes: &BTreeMap<mir::BasicBlock, BTreeSet<String>>,
        introduced_derived_lifetimes: &BTreeMap<
            mir::BasicBlock,
            BTreeMap<String, BTreeSet<String>>,
        >,
        rd_perm: u32,
    ) -> SpannedEncodingResult<()> {
        let entry_label = vir_high::BasicBlockId::new("label_entry".to_string());
        let mut block_builder = procedure_builder.create_basic_block_builder(entry_label.clone());
        if self.mir.basic_blocks().is_empty() {
            block_builder.set_successor_exit(SuccessorExitKind::Return);
        } else {
            block_builder.set_successor_jump(vir_high::Successor::Goto(
                self.encode_basic_block_label(self.mir.start_node()),
            ));
        }
        block_builder.build();
        procedure_builder.set_entry(entry_label);
        let mut merge_blocks: BTreeMap<mir::BasicBlock, mir::BasicBlock> = BTreeMap::new();
        for bb in self.mir.basic_blocks().indices() {
            self.encode_basic_block(
                procedure_builder,
                bb,
                introduced_original_lifetimes,
                introduced_derived_lifetimes,
                rd_perm,
                &mut merge_blocks,
            )?;
        }
        Ok(())
    }

    fn extract_lifetimes_basic_block(
        &mut self,
        bb: mir::BasicBlock,
        bb_introduced_original_lifetimes: &mut BTreeMap<mir::BasicBlock, BTreeSet<String>>,
        bb_introduced_derived_lifetimes: &mut BTreeMap<
            mir::BasicBlock,
            BTreeMap<String, BTreeSet<String>>,
        >,
        all_lifetimes: &mut BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        let mir::BasicBlockData { statements, .. } = &self.mir[bb];
        let mut location = mir::Location {
            block: bb,
            statement_index: 0,
        };
        let terminator_index = statements.len();
        let mut original_lifetimes: BTreeSet<String> = BTreeSet::new();
        let mut derived_lifetimes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        while location.statement_index < terminator_index {
            self.update_lifetimes(
                &mut original_lifetimes,
                &mut derived_lifetimes,
                Some(all_lifetimes),
                location,
            );
            location.statement_index += 1;
        }
        // let mut introduced_original_lifetimes = BTreeSet::new();
        // introduced_original_lifetimes.insert(String::from("bw0"));
        bb_introduced_original_lifetimes.insert(location.block, original_lifetimes);
        bb_introduced_derived_lifetimes.insert(location.block, derived_lifetimes);
        Ok(()) // TODO: do I need this?
    }

    fn encode_basic_block(
        &mut self,
        procedure_builder: &mut ProcedureBuilder,
        bb: mir::BasicBlock,
        introduced_original_lifetimes: &BTreeMap<mir::BasicBlock, BTreeSet<String>>,
        introduced_derived_lifetimes: &BTreeMap<
            mir::BasicBlock,
            BTreeMap<String, BTreeSet<String>>,
        >,
        rd_perm: u32,
        merge_blocks: &mut BTreeMap<mir::BasicBlock, mir::BasicBlock>,
    ) -> SpannedEncodingResult<()> {
        let label = self.encode_basic_block_label(bb);
        let mut block_builder = procedure_builder.create_basic_block_builder(label);
        let mir::BasicBlockData {
            statements,
            terminator,
            ..
        } = &self.mir[bb];
        let mut location = mir::Location {
            block: bb,
            statement_index: 0,
        };
        let terminator_index = statements.len();
        let mut original_lifetimes: BTreeSet<String> = BTreeSet::new();
        let mut derived_lifetimes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        while location.statement_index < terminator_index {
            self.encode_lft(
                &mut block_builder,
                location,
                &mut original_lifetimes,
                &mut derived_lifetimes,
            )?;
            self.encode_statement(
                &mut block_builder,
                location,
                &statements[location.statement_index],
                rd_perm,
            )?;
            location.statement_index += 1;
        }
        // TODO: merging blocks before terminator
        // for this need to know the state of the else branch
        if let Some(terminator) = terminator {
            self.encode_terminator(
                &mut block_builder,
                location,
                terminator,
                introduced_original_lifetimes,
                introduced_derived_lifetimes,
                merge_blocks,
                rd_perm,
            )?;
        }
        block_builder.build();
        Ok(())
    }

    fn encode_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        original_lifetimes: &mut BTreeSet<String>,
        derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        let (new_lifetimes, ended_lifetimes, introduced_derived_lifetimes) =
            self.update_lifetimes(original_lifetimes, derived_lifetimes, None, location);
        self.encode_end_lft(block_builder, location, ended_lifetimes)?;
        self.encode_new_lft(block_builder, location, new_lifetimes)?;
        self.encode_lft_assignment(block_builder, location, introduced_derived_lifetimes)?;
        Ok(())
    }

    fn encode_end_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        lifetimes: BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        for lifetime in lifetimes {
            let lifetime_var = vir_high::VariableDecl::new(lifetime, vir_high::ty::Type::Lifetime);
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::end_lft_no_pos(lifetime_var),
            )?);
        }
        Ok(())
    }

    fn encode_new_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        lifetimes: BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        for lifetime in lifetimes {
            let lifetime_var = vir_high::VariableDecl::new(lifetime, vir_high::ty::Type::Lifetime);
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::new_lft_no_pos(lifetime_var),
            )?);
        }
        Ok(())
    }

    fn encode_shorten_lifetime(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        target: String,
        value: BTreeSet<String>,
        rd_perm: u32,
    ) -> SpannedEncodingResult<()> {
        let encoded_target = vir_high::VariableDecl::new(target, vir_high::ty::Type::Lifetime {});
        block_builder.add_statement(self.set_statement_error(
            location,
            ErrorCtxt::ShortenLifetime,
            vir_high::Statement::shorten_lifetime_no_pos(
                encoded_target,
                value.into_iter().collect(),
                rd_perm,
            ),
        )?);
        Ok(())
    }

    fn encode_ghost_assignment(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        target: String,
        value: BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        let encoded_target = vir_high::VariableDecl::new(target, vir_high::ty::Type::Lifetime {});
        block_builder.add_statement(self.set_statement_error(
            location,
            ErrorCtxt::LifetimeEncoding,
            vir_high::Statement::ghost_assignment_no_pos(
                encoded_target,
                value.into_iter().collect(),
            ),
        )?);
        Ok(())
    }

    fn encode_lft_assignment(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        lifetimes: BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        for (lifetime, constraints) in lifetimes {
            let encoded_target = self.encode_lft_variable(lifetime)?;
            let mut iter = constraints.into_iter();
            let mut intersection = self.encode_lft_variable(iter.next().unwrap())?.into();
            for name in iter {
                intersection = vir_high::Expression::binary_op_no_pos(
                    vir_high::BinaryOpKind::LifetimeIntersection,
                    intersection,
                    self.encode_lft_variable(name)?.into(),
                );
            }
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::ghost_assignment_no_pos(encoded_target, intersection),
            )?);
        }
        Ok(())
    }

    fn encode_lft_variable(
        &self,
        variable_name: String,
    ) -> SpannedEncodingResult<vir_high::VariableDecl> {
        Ok(vir_high::VariableDecl::new(
            variable_name,
            vir_high::Type::Lifetime,
        ))
    }

    fn update_lifetimes(
        &mut self,
        old_original_lifetimes: &mut BTreeSet<String>,
        old_derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
        all_lifetimes: Option<&mut BTreeSet<String>>,
        location: mir::Location,
    ) -> (
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeMap<String, BTreeSet<String>>,
    ) {
        let mut original_lifetimes = self.lifetimes.get_loan_live_at_start(location);
        let derived_lifetimes = self.lifetimes.get_origin_contains_loan_at_mid(location);
        let derived_from: BTreeSet<String> =
            derived_lifetimes.clone().into_values().flatten().collect();

        let lifetimes_to_create: BTreeSet<String> = derived_from
            .clone()
            .into_iter()
            .filter(|x| !old_original_lifetimes.contains(x))
            .collect();

        let lifetimes_to_end: BTreeSet<String> = old_original_lifetimes
            .clone()
            .into_iter()
            .filter(|x| !derived_from.contains(x))
            .collect();

        let derived_lifetimes_to_create: BTreeMap<String, BTreeSet<String>> = derived_lifetimes
            .clone()
            .into_iter()
            .filter(|(k, _)| !old_derived_lifetimes.contains_key(k))
            .collect();

        if let Some(all_lifetimes) = all_lifetimes {
            all_lifetimes.append(&mut original_lifetimes.clone());
            all_lifetimes.append(&mut derived_lifetimes.clone().into_keys().collect());
            // all_lifetimes.append(d.clone());
            // let mut x: u32 = original_lifetimes.len().try_into().unwrap();
            // *lifetime_ctr += x;
            // x = derived_lifetimes.len().try_into().unwrap();
            // *lifetime_ctr += x;
            // // TODO: add the others too
        }

        *old_derived_lifetimes = derived_lifetimes;
        original_lifetimes.append(&mut lifetimes_to_create.clone());
        *old_original_lifetimes = original_lifetimes;

        (
            lifetimes_to_create,
            lifetimes_to_end,
            derived_lifetimes_to_create,
        )
    }

    fn encode_statement(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        statement: &mir::Statement<'tcx>,
        rd_perm: u32,
    ) -> SpannedEncodingResult<()> {
        block_builder.add_comment(format!("{:?} {:?}", location, statement));
        match &statement.kind {
            mir::StatementKind::StorageLive(local) => {
                self.locals_without_explicit_allocation.remove(local);
                let memory_block = self
                    .encoder
                    .encode_memory_block_for_local(self.mir, *local)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::UnexpectedStorageLive,
                    vir_high::Statement::inhale_no_pos(memory_block),
                )?);
                let memory_block_drop = self
                    .encoder
                    .encode_memory_block_drop_for_local(self.mir, *local)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::UnexpectedStorageLive,
                    vir_high::Statement::inhale_no_pos(memory_block_drop),
                )?);
            }
            mir::StatementKind::StorageDead(local) => {
                self.locals_without_explicit_allocation.remove(local);
                let memory_block = self
                    .encoder
                    .encode_memory_block_for_local(self.mir, *local)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::UnexpectedStorageDead,
                    vir_high::Statement::exhale_no_pos(memory_block),
                )?);
                let memory_block_drop = self
                    .encoder
                    .encode_memory_block_drop_for_local(self.mir, *local)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::UnexpectedStorageDead,
                    vir_high::Statement::exhale_no_pos(memory_block_drop),
                )?);
            }
            mir::StatementKind::Assign(box (target, source)) => {
                let position = self.register_error(location, ErrorCtxt::Unexpected);
                let encoded_target = self
                    .encoder
                    .encode_place_high(self.mir, *target)?
                    .set_default_position(position);
                self.encode_statement_assign(
                    block_builder,
                    location,
                    encoded_target,
                    source,
                    rd_perm,
                )?;
            }
            _ => {
                block_builder.add_comment("encode_statement: not encoded".to_string());
            }
        }
        Ok(())
    }

    fn encode_statement_assign(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        encoded_target: vir_crate::high::Expression,
        source: &mir::Rvalue<'tcx>,
        rd_perm: u32,
    ) -> SpannedEncodingResult<()> {
        match source {
            mir::Rvalue::Use(operand) => {
                self.encode_assign_operand(block_builder, location, encoded_target, operand)?;
            }
            // mir::Rvalue::Repeat(Operand<'tcx>, Const<'tcx>),
            mir::Rvalue::Ref(region, borrow_kind, place) => {
                let is_mut = matches!(
                    borrow_kind,
                    mir::BorrowKind::Mut {
                        allow_two_phase_borrow: _,
                    }
                );
                let encoded_place = self.encoder.encode_place_high(self.mir, *place)?;
                let region_name = region.to_text();
                let encoded_rvalue = vir_high::Rvalue::ref_(
                    encoded_place,
                    vir_high::ty::LifetimeConst::new(region_name),
                    is_mut,
                    rd_perm,
                    encoded_target.clone(),
                );
                let assign_statement = vir_high::Statement::assign(
                    encoded_target,
                    encoded_rvalue,
                    self.register_error(location, ErrorCtxt::Assign),
                );
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::Assign,
                    assign_statement,
                )?);
            }
            // mir::Rvalue::ThreadLocalRef(DefId),
            mir::Rvalue::AddressOf(_, place) => {
                let encoded_place = self.encoder.encode_place_high(self.mir, *place)?;
                let encoded_rvalue = vir_high::Rvalue::address_of(encoded_place);
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::Assign,
                    vir_high::Statement::assign_no_pos(encoded_target, encoded_rvalue),
                )?);
            }
            // mir::Rvalue::Len(Place<'tcx>),
            // mir::Rvalue::Cast(CastKind, Operand<'tcx>, Ty<'tcx>),
            mir::Rvalue::BinaryOp(op, box (left, right)) => {
                let encoded_left = self.encode_statement_operand(location, left)?;
                let encoded_right = self.encode_statement_operand(location, right)?;
                let kind = self.encode_binary_op_kind(*op)?;
                let encoded_rvalue = vir_high::Rvalue::binary_op(kind, encoded_left, encoded_right);
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::Assign,
                    vir_high::Statement::assign_no_pos(encoded_target, encoded_rvalue),
                )?);
            }
            mir::Rvalue::CheckedBinaryOp(op, box (left, right)) => {
                let encoded_left = self.encode_statement_operand(location, left)?;
                let encoded_right = self.encode_statement_operand(location, right)?;
                let kind = self.encode_binary_op_kind(*op)?;
                let encoded_rvalue =
                    vir_high::Rvalue::checked_binary_op(kind, encoded_left, encoded_right);
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::Assign,
                    vir_high::Statement::assign_no_pos(encoded_target, encoded_rvalue),
                )?);
            }
            // mir::Rvalue::NullaryOp(NullOp, Ty<'tcx>),
            mir::Rvalue::UnaryOp(op, operand) => {
                let encoded_operand = self.encode_statement_operand(location, operand)?;
                let kind = match op {
                    mir::UnOp::Not => vir_high::UnaryOpKind::Not,
                    mir::UnOp::Neg => vir_high::UnaryOpKind::Minus,
                };
                let encoded_rvalue = vir_high::Rvalue::unary_op(kind, encoded_operand);
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::Assign,
                    vir_high::Statement::assign_no_pos(encoded_target, encoded_rvalue),
                )?);
            }
            mir::Rvalue::Discriminant(place) => {
                let encoded_place = self.encoder.encode_place_high(self.mir, *place)?;
                let encoded_rvalue = vir_high::Rvalue::discriminant(encoded_place);
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::Assign,
                    vir_high::Statement::assign_no_pos(encoded_target, encoded_rvalue),
                )?);
            }
            mir::Rvalue::Aggregate(box aggregate_kind, operands) => {
                self.encode_statement_assign_aggregate(
                    block_builder,
                    location,
                    encoded_target,
                    aggregate_kind,
                    operands,
                )?;
            }
            // mir::Rvalue::ShallowInitBox(Operand<'tcx>, Ty<'tcx>),
            _ => {
                block_builder.add_comment("encode_statement_assign: not encoded".to_string());
                unimplemented!("{:?}", source);
            }
        }
        Ok(())
    }

    fn encode_statement_assign_aggregate(
        &self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        encoded_target: vir_crate::high::Expression,
        aggregate_kind: &mir::AggregateKind<'tcx>,
        operands: &[mir::Operand<'tcx>],
    ) -> SpannedEncodingResult<()> {
        let ty = match aggregate_kind {
            mir::AggregateKind::Array(_) => unimplemented!(),
            mir::AggregateKind::Tuple => encoded_target.get_type().clone(),
            mir::AggregateKind::Adt(adt_did, variant_index, _substs, _, active_field_index) => {
                let mut ty = encoded_target.get_type().clone();
                let tcx = self.encoder.env().tcx();
                if ty.is_enum() {
                    let adt_def = tcx.adt_def(*adt_did);
                    let variant_def = &adt_def.variants()[*variant_index];
                    let variant_name = variant_def.ident(tcx).to_string();
                    ty = ty.variant(variant_name.into());
                } else {
                    assert_eq!(
                        variant_index.index(),
                        0,
                        "Unexpected value of the variant index."
                    );
                }
                if let Some(active_field_index) = active_field_index {
                    assert!(ty.is_union());
                    let adt_def = tcx.adt_def(*adt_did);
                    let variant_def = adt_def.non_enum_variant();
                    let field_name = variant_def.fields[*active_field_index]
                        .ident(tcx)
                        .to_string();
                    ty = ty.variant(field_name.into());
                }
                ty
            }
            mir::AggregateKind::Closure(_, _) => unimplemented!(),
            mir::AggregateKind::Generator(_, _, _) => unimplemented!(),
        };
        let mut encoded_operands = Vec::new();
        for operand in operands {
            encoded_operands.push(self.encode_statement_operand(location, operand)?);
        }
        let encoded_rvalue = vir_high::Rvalue::aggregate(ty, encoded_operands);
        block_builder.add_statement(vir_high::Statement::assign(
            encoded_target,
            encoded_rvalue,
            self.register_error(location, ErrorCtxt::Assign),
        ));
        Ok(())
    }

    fn encode_assign_operand(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        encoded_target: vir_crate::high::Expression,
        operand: &mir::Operand<'tcx>,
    ) -> SpannedEncodingResult<()> {
        let span = self.encoder.get_span_of_location(self.mir, location);
        match operand {
            mir::Operand::Move(source) => {
                let encoded_source = self.encoder.encode_place_high(self.mir, *source)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::MovePlace,
                    vir_high::Statement::move_place_no_pos(encoded_target, encoded_source),
                )?);
            }
            mir::Operand::Copy(source) => {
                let encoded_source = self.encoder.encode_place_high(self.mir, *source)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::CopyPlace,
                    vir_high::Statement::copy_place_no_pos(encoded_target, encoded_source),
                )?);
            }
            mir::Operand::Constant(constant) => {
                let encoded_constant = self
                    .encoder
                    .encode_constant_high(constant)
                    .with_span(span)?;
                block_builder.add_statement(self.set_statement_error(
                    location,
                    ErrorCtxt::WritePlace,
                    vir_high::Statement::write_place_no_pos(encoded_target, encoded_constant),
                )?);
            }
        }
        Ok(())
    }

    fn encode_statement_operand(
        &self,
        location: mir::Location,
        operand: &mir::Operand<'tcx>,
    ) -> SpannedEncodingResult<vir_high::Operand> {
        let span = self.encoder.get_span_of_location(self.mir, location);
        let encoded_operand = match operand {
            mir::Operand::Move(source) => {
                let position = self.register_error(location, ErrorCtxt::MovePlace);
                let encoded_source = self
                    .encoder
                    .encode_place_high(self.mir, *source)?
                    .set_default_position(position);
                vir_high::Operand::new(vir_high::OperandKind::Move, encoded_source)
            }
            mir::Operand::Copy(source) => {
                let position = self.register_error(location, ErrorCtxt::CopyPlace);
                let encoded_source = self
                    .encoder
                    .encode_place_high(self.mir, *source)?
                    .set_default_position(position);
                vir_high::Operand::new(vir_high::OperandKind::Copy, encoded_source)
            }
            mir::Operand::Constant(constant) => {
                let position = self.register_error(location, ErrorCtxt::WritePlace);
                let encoded_constant = self
                    .encoder
                    .encode_constant_high(constant)
                    .with_span(span)?
                    .set_default_position(position);
                vir_high::Operand::new(vir_high::OperandKind::Constant, encoded_constant)
            }
        };
        Ok(encoded_operand)
    }

    fn encode_binary_op_kind(
        &self,
        op: mir::BinOp,
    ) -> SpannedEncodingResult<vir_high::BinaryOpKind> {
        let kind = match op {
            mir::BinOp::Add => vir_high::BinaryOpKind::Add,
            mir::BinOp::Sub => vir_high::BinaryOpKind::Sub,
            mir::BinOp::Mul => vir_high::BinaryOpKind::Mul,
            mir::BinOp::Div => vir_high::BinaryOpKind::Div,
            mir::BinOp::Rem => vir_high::BinaryOpKind::Mod,
            // mir::BinOp::BitXor => vir_high::BinaryOpKind::BitXor,
            // mir::BinOp::BitAnd => vir_high::BinaryOpKind::BitAnd,
            // mir::BinOp::BitOr => vir_high::BinaryOpKind::BitOr,
            // mir::BinOp::Shl => vir_high::BinaryOpKind::Shl,
            // mir::BinOp::Shr => vir_high::BinaryOpKind::Shr,
            mir::BinOp::Eq => vir_high::BinaryOpKind::EqCmp,
            mir::BinOp::Lt => vir_high::BinaryOpKind::LtCmp,
            mir::BinOp::Le => vir_high::BinaryOpKind::LeCmp,
            mir::BinOp::Ne => vir_high::BinaryOpKind::NeCmp,
            mir::BinOp::Ge => vir_high::BinaryOpKind::GeCmp,
            mir::BinOp::Gt => vir_high::BinaryOpKind::GtCmp,
            // mir::BinOp::Offset => vir_high::BinaryOpKind::Offset,
            _ => unimplemented!("op kind: {:?}", op),
        };
        Ok(kind)
    }

    fn encode_merge_blocks(
        &mut self,
        location: mir::Location,
        block_builder: &mut BasicBlockBuilder,
        introduced_original_lifetimes: &BTreeMap<mir::BasicBlock, BTreeSet<String>>,
        introduced_derived_lifetimes: &BTreeMap<
            mir::BasicBlock,
            BTreeMap<String, BTreeSet<String>>,
        >,
        merge_blocks: &mut BTreeMap<mir::BasicBlock, mir::BasicBlock>,
        rd_perm: u32,
    ) -> SpannedEncodingResult<()> {
        let brother_block = merge_blocks.get(&location.block);
        if let Some(brother_block) = brother_block {
            let lifetimes = introduced_original_lifetimes
                .get(brother_block)
                .unwrap()
                .clone();
            self.encode_new_lft(block_builder, location, lifetimes)?;

            // TODO: this is ugly, how do I write this nicely in rust? o.O
            if introduced_derived_lifetimes.contains_key(brother_block) {
                let derived_brother_block =
                    introduced_derived_lifetimes.get(brother_block).unwrap();
                let derived_block = if introduced_derived_lifetimes.contains_key(&location.block) {
                    introduced_derived_lifetimes
                        .get(&location.block)
                        .unwrap()
                        .clone()
                } else {
                    BTreeMap::new()
                };
                for (target, val) in derived_brother_block {
                    let mut value: BTreeSet<String> = val.clone();
                    if derived_block.contains_key(target) {
                        value.append(derived_block.get(target).unwrap().clone().borrow_mut());
                        self.encode_shorten_lifetime(
                            block_builder,
                            location,
                            target.clone(),
                            value,
                            rd_perm,
                        )?;
                    } else {
                        self.encode_ghost_assignment(
                            block_builder,
                            location,
                            target.clone(),
                            value,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_terminator(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        terminator: &mir::Terminator<'tcx>,
        introduced_original_lifetimes: &BTreeMap<mir::BasicBlock, BTreeSet<String>>,
        introduced_derived_lifetimes: &BTreeMap<
            mir::BasicBlock,
            BTreeMap<String, BTreeSet<String>>,
        >,
        merge_blocks: &mut BTreeMap<mir::BasicBlock, mir::BasicBlock>,
        rd_perm: u32,
    ) -> SpannedEncodingResult<()> {
        block_builder.add_comment(format!("{:?} {:?}", location, terminator.kind));
        let span = self.encoder.get_span_of_location(self.mir, location);
        use rustc_middle::mir::TerminatorKind;
        let successor = match &terminator.kind {
            TerminatorKind::Goto { target } => {
                self.encode_merge_blocks(
                    location,
                    block_builder,
                    introduced_original_lifetimes,
                    introduced_derived_lifetimes,
                    merge_blocks,
                    rd_perm,
                )?;
                SuccessorBuilder::jump(vir_high::Successor::Goto(
                    self.encode_basic_block_label(*target),
                ))
            }
            TerminatorKind::SwitchInt {
                targets,
                discr,
                switch_ty,
            } => {
                let blocks = targets.all_targets();
                merge_blocks.insert(blocks[0], blocks[1]);
                merge_blocks.insert(blocks[1], blocks[0]);
                self.encode_terminator_switch_int(span, targets, discr, *switch_ty)?
            }
            TerminatorKind::Resume => SuccessorBuilder::exit_resume_panic(),
            // TerminatorKind::Abort => {
            //     graph.add_exit_edge(bb, "abort");
            // }
            TerminatorKind::Return => SuccessorBuilder::exit_return(),
            TerminatorKind::Unreachable => {
                block_builder
                    .add_comment("Target marked as unreachable by the compiler".to_string());
                block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                    vir_high::Statement::assert_no_pos(false.into()),
                    span,
                    ErrorCtxt::UnreachableTerminator,
                    self.def_id,
                )?);
                SuccessorBuilder::exit_resume_panic()
            }
            // TerminatorKind::DropAndReplace { target, unwind, .. }
            // | TerminatorKind::Drop { target, unwind, .. } => {
            //     graph.add_regular_edge(bb, target);
            //     if let Some(target) = unwind {
            //         graph.add_unwind_edge(bb, target);
            //     }
            // }
            TerminatorKind::Call {
                func: mir::Operand::Constant(box mir::Constant { literal, .. }),
                args,
                destination,
                cleanup,
                fn_span,
                from_hir_call: _,
            } => {
                self.encode_terminator_call(
                    block_builder,
                    location,
                    span,
                    literal.ty(),
                    args,
                    destination,
                    cleanup,
                    *fn_span,
                )?;
                // The encoding of the call is expected to set the successor.
                return Ok(());
            }
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
                cleanup,
            } => {
                // TODO: Move this thing to a method.
                let condition = self
                    .encoder
                    .encode_operand_high(self.mir, cond)
                    .with_default_span(span)?;

                let guard = if *expected {
                    condition
                } else {
                    vir_high::Expression::not(condition)
                };

                let (assert_msg, error_ctxt) = if let mir::AssertKind::BoundsCheck { .. } = msg {
                    let mut s = String::new();
                    msg.fmt_assert_args(&mut s).unwrap();
                    (s, ErrorCtxt::BoundsCheckAssert)
                } else {
                    let assert_msg = msg.description().to_string();
                    (assert_msg.clone(), ErrorCtxt::AssertTerminator(assert_msg))
                };

                let target_label = self.encode_basic_block_label(*target);
                block_builder.add_comment(format!("Rust assertion: {}", assert_msg));
                if self.check_panics {
                    block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                        vir_high::Statement::assert_no_pos(guard.clone()),
                        span,
                        error_ctxt,
                        self.def_id,
                    )?);
                }
                if let Some(cleanup) = cleanup {
                    let successors = vec![
                        (guard, target_label),
                        (true.into(), self.encode_basic_block_label(*cleanup)),
                    ];
                    SuccessorBuilder::jump(vir_high::Successor::GotoSwitch(successors))
                } else {
                    SuccessorBuilder::jump(vir_high::Successor::Goto(target_label))
                }
            }
            // TerminatorKind::Yield { .. } => {
            //     graph.add_exit_edge(bb, "yield");
            // }
            // TerminatorKind::GeneratorDrop => {
            //     graph.add_exit_edge(bb, "generator_drop");
            // }
            TerminatorKind::FalseEdge {
                real_target,
                imaginary_target: _,
            } => SuccessorBuilder::jump(vir_high::Successor::Goto(
                self.encode_basic_block_label(*real_target),
            )),
            // TerminatorKind::FalseUnwind {
            //     real_target,
            //     unwind,
            // } => {
            //     graph.add_regular_edge(bb, real_target);
            //     if let Some(imaginary_target) = unwind {
            //         graph.add_imaginary_edge(bb, imaginary_target);
            //     }
            // }
            // TerminatorKind::InlineAsm { .. } => {
            //     graph.add_exit_edge(bb, "inline_asm");
            // }
            x => unimplemented!("terminator: {:?}", x),
        };
        block_builder.set_successor(successor);
        Ok(())
    }

    fn encode_terminator_switch_int(
        &self,
        span: Span,
        targets: &mir::SwitchTargets,
        discr: &mir::Operand<'tcx>,
        switch_ty: ty::Ty<'tcx>,
    ) -> SpannedEncodingResult<SuccessorBuilder> {
        let discriminant = self
            .encoder
            .encode_operand_high(self.mir, discr)
            .with_default_span(span)?;
        debug!(
            "discriminant: {:?} switch_ty: {:?}",
            discriminant, switch_ty
        );
        let mut successors = Vec::new();
        for (value, target) in targets.iter() {
            let encoded_condition = match switch_ty.kind() {
                ty::TyKind::Bool => {
                    if value == 0 {
                        // If discr is 0 (false)
                        vir_high::Expression::not(discriminant.clone())
                    } else {
                        // If discr is not 0 (true)
                        discriminant.clone()
                    }
                }
                ty::TyKind::Int(_) | ty::TyKind::Uint(_) | ty::TyKind::Char => {
                    vir_high::Expression::equals(
                        discriminant.clone(),
                        self.encoder
                            .encode_int_cast_high(value, switch_ty)
                            .with_span(span)?,
                    )
                }
                x => unreachable!("{:?}", x),
            };
            let encoded_target = self.encode_basic_block_label(target);
            successors.push((encoded_condition, encoded_target));
        }
        successors.push((
            true.into(),
            self.encode_basic_block_label(targets.otherwise()),
        ));
        Ok(SuccessorBuilder::jump(vir_high::Successor::GotoSwitch(
            successors,
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_terminator_call(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        span: Span,
        ty: ty::Ty<'tcx>,
        args: &[mir::Operand<'tcx>],
        destination: &Option<(mir::Place<'tcx>, mir::BasicBlock)>,
        cleanup: &Option<mir::BasicBlock>,
        _fn_span: Span,
    ) -> SpannedEncodingResult<()> {
        if let ty::TyKind::FnDef(def_id, _substs) = ty.kind() {
            let full_called_function_name = self.encoder.env().tcx().def_path_str(*def_id);
            if !self.try_encode_builtin_call(
                block_builder,
                span,
                &full_called_function_name,
                args,
                destination,
                cleanup,
            )? {
                if let ty::TyKind::FnDef(called_def_id, call_substs) = ty.kind() {
                    self.encode_function_call(
                        block_builder,
                        location,
                        span,
                        *called_def_id,
                        call_substs,
                        args,
                        destination,
                        cleanup,
                    )?;
                } else {
                    // Other kind of calls?
                    unimplemented!();
                }
            }
        } else {
            unimplemented!();
        };
        Ok(())
    }

    fn try_encode_builtin_call(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        span: Span,
        called_function: &str,
        args: &[mir::Operand<'tcx>],
        destination: &Option<(mir::Place<'tcx>, mir::BasicBlock)>,
        cleanup: &Option<mir::BasicBlock>,
    ) -> SpannedEncodingResult<bool> {
        match called_function {
            "core::panicking::panic" => {
                let panic_message = format!("{:?}", args[0]);
                let panic_cause = self.encoder.encode_panic_cause(span)?;
                if self.check_panics {
                    block_builder.add_comment(format!("Rust panic - {}", panic_message));
                    block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                        vir_high::Statement::assert_no_pos(false.into()),
                        span,
                        ErrorCtxt::Panic(panic_cause),
                        self.def_id,
                    )?);
                } else {
                    debug!("Absence of panic will not be checked")
                }
            }
            _ => return Ok(false),
        }
        if let Some(destination) = destination {
            unimplemented!("destination: {:?}", destination);
        }
        if let Some(cleanup) = cleanup {
            block_builder.set_successor_jump(vir_high::Successor::Goto(
                self.encode_basic_block_label(*cleanup),
            ))
        } else {
            unimplemented!();
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_function_call(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        span: Span,
        called_def_id: DefId,
        call_substs: SubstsRef<'tcx>,
        args: &[mir::Operand<'tcx>],
        destination: &Option<(mir::Place<'tcx>, mir::BasicBlock)>,
        cleanup: &Option<mir::BasicBlock>,
    ) -> SpannedEncodingResult<()> {
        // The called method might be a trait method.
        // We try to resolve it to the concrete implementation
        // and type substitutions.
        let (called_def_id, call_substs) =
            self.encoder
                .env()
                .resolve_method_call(self.def_id, called_def_id, call_substs);

        let old_label = self.fresh_old_label();
        block_builder.add_statement(self.encoder.set_statement_error_ctxt(
            vir_high::Statement::old_label_no_pos(old_label.clone()),
            span,
            ErrorCtxt::ProcedureCall,
            self.def_id,
        )?);
        let mut arguments = Vec::new();
        for arg in args {
            arguments.push(
                self.encoder
                    .encode_operand_high(self.mir, arg)
                    .with_span(span)?,
            );
            let encoded_arg = self.encode_statement_operand(location, arg)?;
            let statement = vir_high::Statement::consume_no_pos(encoded_arg);
            block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                statement,
                span,
                ErrorCtxt::ProcedureCall,
                self.def_id,
            )?);
        }

        let procedure_contract = self
            .encoder
            .get_mir_procedure_contract_for_call(self.def_id, called_def_id, call_substs)
            .with_span(span)?;

        for expression in
            self.encode_precondition_expressions(&procedure_contract, call_substs, &arguments)?
        {
            let assert_statement = self.encoder.set_statement_error_ctxt(
                vir_high::Statement::assert_no_pos(expression),
                span,
                ErrorCtxt::ExhaleMethodPrecondition,
                self.def_id,
            )?;
            block_builder.add_statement(assert_statement);
        }

        if self.encoder.env().tcx().is_closure(called_def_id) {
            // Closure calls are wrapped around std::ops::Fn::call(), which receives
            // two arguments: The closure instance, and the tupled-up arguments
            assert_eq!(args.len(), 2);
            unimplemented!();
        }

        if let Some((target_place, target_block)) = destination {
            let position = self.register_error(location, ErrorCtxt::ProcedureCall);
            let encoded_target_place = self
                .encoder
                .encode_place_high(self.mir, *target_place)?
                .set_default_position(position);
            let postcondition_expressions = self.encode_postcondition_expressions(
                &procedure_contract,
                call_substs,
                arguments.clone(),
                &encoded_target_place,
                &old_label,
            )?;
            if let Some(target_place_local) = target_place.as_local() {
                let size = self.encoder.encode_type_size_expression(
                    self.encoder.get_local_type(self.mir, target_place_local)?,
                )?;
                let target_memory_block = vir_high::Predicate::memory_block_stack_no_pos(
                    encoded_target_place.clone(),
                    size,
                );
                block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                    vir_high::Statement::exhale_no_pos(target_memory_block.clone()),
                    span,
                    ErrorCtxt::ProcedureCall,
                    self.def_id,
                )?);
                let target_label = self.encode_basic_block_label(*target_block);

                let fresh_destination_label = self.fresh_basic_block_label();
                let mut post_call_block_builder =
                    block_builder.create_basic_block_builder(fresh_destination_label.clone());
                post_call_block_builder.set_successor_jump(vir_high::Successor::Goto(target_label));
                let statement = vir_high::Statement::inhale_no_pos(
                    vir_high::Predicate::owned_non_aliased_no_pos(encoded_target_place.clone()),
                );
                post_call_block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                    statement,
                    span,
                    ErrorCtxt::ProcedureCall,
                    self.def_id,
                )?);
                for expression in postcondition_expressions {
                    let assume_statement = self.encoder.set_statement_error_ctxt(
                        vir_high::Statement::assume_no_pos(expression),
                        span,
                        ErrorCtxt::UnexpectedAssumeMethodPostcondition,
                        self.def_id,
                    )?;
                    post_call_block_builder.add_statement(assume_statement);
                }
                if self.encoder.is_pure(called_def_id, Some(call_substs))
                    && self.def_id != called_def_id
                {
                    // If we are verifying a pure function, we always need
                    // to encode it as a method
                    //
                    // FIXME: This is not enough, we also need to handle
                    // mutual-recursion case.
                    let (function_name, return_type) = self
                        .encoder
                        .encode_pure_function_use_high(called_def_id, self.def_id, call_substs)
                        .with_span(span)?;
                    let type_arguments = self
                        .encoder
                        .encode_generic_arguments_high(called_def_id, call_substs)
                        .with_span(span)?;
                    let expression = vir_high::Expression::equals(
                        encoded_target_place,
                        vir_high::Expression::function_call(
                            function_name,
                            type_arguments,
                            arguments,
                            return_type,
                        ),
                    );
                    let assume_statement = self.encoder.set_statement_error_ctxt(
                        vir_high::Statement::assume_no_pos(expression),
                        span,
                        ErrorCtxt::UnexpectedAssumeMethodPostcondition,
                        self.def_id,
                    )?;
                    post_call_block_builder.add_statement(assume_statement);
                }
                post_call_block_builder.build();

                if let Some(cleanup_block) = cleanup {
                    let encoded_cleanup_block = self.encode_basic_block_label(*cleanup_block);
                    let fresh_cleanup_label = self.fresh_basic_block_label();
                    let mut cleanup_block_builder =
                        block_builder.create_basic_block_builder(fresh_cleanup_label.clone());
                    cleanup_block_builder
                        .set_successor_jump(vir_high::Successor::Goto(encoded_cleanup_block));

                    let statement = vir_high::Statement::inhale_no_pos(target_memory_block);
                    cleanup_block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                        statement,
                        span,
                        ErrorCtxt::ProcedureCall,
                        self.def_id,
                    )?);
                    cleanup_block_builder.build();
                    block_builder.set_successor_jump(vir_high::Successor::NonDetChoice(
                        fresh_destination_label,
                        fresh_cleanup_label,
                    ));
                } else {
                    unimplemented!();
                }
            } else {
                unimplemented!();
            }
        } else if let Some(_cleanup_block) = cleanup {
            // TODO: add panic postconditions.
            unimplemented!();
        } else {
            // TODO: Can we always soundly assume false here?
            unimplemented!();
        }

        Ok(())
    }

    fn encode_basic_block_label(&self, bb: mir::BasicBlock) -> vir_high::BasicBlockId {
        vir_high::BasicBlockId::new(format!("label_{:?}", bb))
    }

    fn fresh_basic_block_label(&mut self) -> vir_high::BasicBlockId {
        let name = format!("label_{}_custom", self.fresh_id_generator);
        self.fresh_id_generator += 1;
        vir_high::BasicBlockId::new(name)
    }

    fn fresh_old_label(&mut self) -> String {
        let name = format!("label_{}_old", self.fresh_id_generator);
        self.fresh_id_generator += 1;
        name
    }

    fn register_error(&self, location: mir::Location, error_ctxt: ErrorCtxt) -> vir_high::Position {
        let span = self.encoder.get_mir_location_span(self.mir, location);
        self.encoder
            .error_manager()
            .register_error(span, error_ctxt, self.def_id)
            .into()
    }

    fn set_statement_error(
        &mut self,
        location: mir::Location,
        error_ctxt: ErrorCtxt,
        statement: vir_high::Statement,
    ) -> SpannedEncodingResult<vir_high::Statement> {
        let position = self.register_error(location, error_ctxt.clone());
        self.encoder
            .set_statement_error_ctxt_from_position(statement, position, error_ctxt)
    }
}
