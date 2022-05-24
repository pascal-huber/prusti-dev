use crate::encoder::{
    errors::{ErrorCtxt, SpannedEncodingResult},
    mir::{
        errors::ErrorInterface,
        procedures::encoder::{scc::*, ProcedureEncoder},
    },
};
use prusti_interface::environment::mir_dump::graphviz::ToText;
use rustc_middle::mir;
use std::collections::{BTreeMap, BTreeSet};
use vir_crate::high::{self as vir_high, builders::procedure::BasicBlockBuilder};

pub(super) trait LifetimesEncoder {
    fn encode_lft_for_statement(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        original_lifetimes: &mut BTreeSet<String>,
        derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft_for_block(
        &mut self,
        target: mir::BasicBlock,
        location: mir::Location,
        block_builder: &mut BasicBlockBuilder,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_original_lifetimes: &mut BTreeSet<String>,
        old_derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()>;
    fn encode_dead_variables(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()>;
    fn encode_new_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> SpannedEncodingResult<()>;
    fn encode_end_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft_return(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft_take(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft_assert_subset(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        lifetime_lhs: String,
        lifetime_rhs: String,
    ) -> SpannedEncodingResult<()>;
    fn encode_lft_variable(
        &self,
        variable_name: String,
    ) -> SpannedEncodingResult<vir_high::VariableDecl>;
    fn variables_to_kill(
        &mut self,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<BTreeSet<vir_high::Local>>;
    fn lifetimes_to_return(
        &mut self,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> BTreeMap<String, BTreeSet<String>>;
    fn lifetimes_to_take(
        &mut self,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> BTreeMap<String, BTreeSet<String>>;
    fn lifetimes_to_end(
        &mut self,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> BTreeSet<String>;
    fn lifetimes_to_create(
        &mut self,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> BTreeSet<String>;
    fn none_permission(&self) -> vir_high::Expression;
    fn full_permission(&self) -> vir_high::Expression;
    fn encode_lifetime_specifications(
        &mut self,
    ) -> SpannedEncodingResult<(Vec<vir_high::Statement>, Vec<vir_high::Statement>)>;
    fn get_lifetime_name(&mut self, variable: vir_high::Expression) -> Option<String>;
    fn identical_lifetimes(
        &mut self,
        existing_lifetimes: BTreeSet<String>,
        relations: BTreeSet<(String, String)>,
    ) -> BTreeMap<String, String>;
    fn lifetimes_to_inhale(
        &mut self,
    ) -> SpannedEncodingResult<BTreeSet<vir_high::ty::LifetimeConst>>;
    fn lifetimes_to_exhale_inhale_function(
        &mut self,
    ) -> SpannedEncodingResult<BTreeSet<vir_high::ty::LifetimeConst>>;
    fn encode_inhale_lifetime_token(
        &mut self,
        lifetime_const: vir_high::ty::LifetimeConst,
        rd_perm: u32,
    ) -> SpannedEncodingResult<vir_high::Statement>;
    fn encode_exhale_lifetime_token(
        &mut self,
        lifetime_const: vir_high::ty::LifetimeConst,
    ) -> SpannedEncodingResult<vir_high::Statement>;
}

impl<'p, 'v: 'p, 'tcx: 'v> LifetimesEncoder for ProcedureEncoder<'p, 'v, 'tcx> {
    fn encode_lft_for_statement(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        original_lifetimes: &mut BTreeSet<String>,
        derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        let new_derived_lifetimes = self.lifetimes.get_origin_contains_loan_at_mid(location);
        block_builder.add_comment(format!("Prepare lifetimes for statement {:?}", location));
        self.encode_lft(
            block_builder,
            location,
            original_lifetimes,
            derived_lifetimes,
            &new_derived_lifetimes,
        )?;
        Ok(())
    }

    fn encode_lft_for_block(
        &mut self,
        target: mir::BasicBlock,
        location: mir::Location,
        block_builder: &mut BasicBlockBuilder,
    ) -> SpannedEncodingResult<()> {
        let needed_derived_lifetimes = self.needed_derived_lifetimes_for_block(&target);
        let mut current_derived_lifetimes =
            self.lifetimes.get_origin_contains_loan_at_mid(location);
        let mut current_original_lifetimes = self.lifetimes.get_loan_live_at_start(location);
        block_builder.add_comment(format!("Prepare lifetimes for block {:?}", target));
        self.encode_lft(
            block_builder,
            location,
            &mut current_original_lifetimes,
            &mut current_derived_lifetimes,
            &needed_derived_lifetimes,
        )?;
        Ok(())
    }

    /// Adds all statements needed for the encoding of the location.
    fn encode_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_original_lifetimes: &mut BTreeSet<String>,
        old_derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        let new_original_lifetimes: BTreeSet<String> = new_derived_lifetimes
            .clone()
            .into_values()
            .flatten()
            .collect();
        self.encode_lft_return(
            block_builder,
            location,
            old_derived_lifetimes,
            new_derived_lifetimes,
        )?;
        self.encode_end_lft(
            block_builder,
            location,
            old_original_lifetimes,
            &new_original_lifetimes,
        )?;
        self.encode_dead_variables(
            block_builder,
            location,
            old_derived_lifetimes,
            new_derived_lifetimes,
        )?;
        self.encode_new_lft(
            block_builder,
            location,
            old_original_lifetimes,
            &new_original_lifetimes,
        )?;
        self.encode_lft_take(
            block_builder,
            location,
            old_derived_lifetimes,
            new_derived_lifetimes,
        )?;

        *old_original_lifetimes = new_original_lifetimes.clone();
        *old_derived_lifetimes = new_derived_lifetimes.clone();
        Ok(())
    }

    fn encode_dead_variables(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        let variables_to_kill =
            self.variables_to_kill(old_derived_lifetimes, new_derived_lifetimes)?;
        for var in variables_to_kill {
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::dead_no_pos(var.into()),
            )?);
        }
        Ok(())
    }

    fn encode_new_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        let lifetimes_to_create =
            self.lifetimes_to_create(old_original_lifetimes, new_original_lifetimes);
        for lifetime in lifetimes_to_create {
            let lifetime_var = vir_high::VariableDecl::new(lifetime, vir_high::ty::Type::Lifetime);
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::new_lft_no_pos(lifetime_var),
            )?);
        }
        Ok(())
    }

    fn encode_end_lft(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> SpannedEncodingResult<()> {
        let lifetimes_to_end: BTreeSet<String> =
            self.lifetimes_to_end(old_original_lifetimes, new_original_lifetimes);
        for lifetime in lifetimes_to_end {
            let lifetime_var = vir_high::VariableDecl::new(lifetime, vir_high::ty::Type::Lifetime);
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::end_lft_no_pos(lifetime_var),
            )?);
        }
        Ok(())
    }

    fn encode_lft_return(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        let lifetimes_to_return =
            self.lifetimes_to_return(old_derived_lifetimes, new_derived_lifetimes);
        for (lifetime, derived_from) in lifetimes_to_return {
            let encoded_target = self.encode_lft_variable(lifetime)?;
            let mut lifetimes: Vec<vir_high::VariableDecl> = Vec::new();
            for lifetime_name in derived_from {
                lifetimes.push(self.encode_lft_variable(lifetime_name)?);
            }
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::lifetime_return_no_pos(
                    encoded_target,
                    lifetimes,
                    self.rd_perm,
                ),
            )?);
        }
        Ok(())
    }

    fn encode_lft_take(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<()> {
        let lifetimes_to_take =
            self.lifetimes_to_take(old_derived_lifetimes, new_derived_lifetimes);
        for (lifetime, derived_from) in lifetimes_to_take {
            let encoded_target = self.encode_lft_variable(lifetime)?;
            let mut lifetimes: Vec<vir_high::VariableDecl> = Vec::new();
            for lifetime_name in derived_from {
                lifetimes.push(self.encode_lft_variable(lifetime_name)?);
            }
            block_builder.add_statement(self.set_statement_error(
                location,
                ErrorCtxt::LifetimeEncoding,
                vir_high::Statement::lifetime_take_no_pos(encoded_target, lifetimes, self.rd_perm),
            )?);
        }
        Ok(())
    }

    fn encode_lft_assert_subset(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        lifetime_lhs: String,
        lifetime_rhs: String,
    ) -> SpannedEncodingResult<()> {
        let lhs = vir_high::ty::LifetimeConst { name: lifetime_lhs };
        let rhs = vir_high::ty::LifetimeConst { name: lifetime_rhs };
        let assert_statement = vir_high::Statement::lifetime_included_no_pos(
            true, // assert, not assume
            lhs,
            vec![rhs],
        );
        block_builder.add_statement(self.set_statement_error(
            location,
            ErrorCtxt::LifetimeEncoding,
            assert_statement,
        )?);
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

    /// A variable can be killed if its lifetime was previously derived from lifetimes
    /// but isn't anymore now.
    fn variables_to_kill(
        &mut self,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<BTreeSet<vir_high::Local>> {
        let mut variables_to_kill = BTreeSet::new();
        for derived_lifetime in old_derived_lifetimes.keys() {
            if !new_derived_lifetimes.contains_key(derived_lifetime) {
                if let Some(local) = self.procedure.get_var_of_lifetime(&derived_lifetime[..]) {
                    variables_to_kill.insert(self.encode_local(local)?);
                }
            }
        }
        Ok(variables_to_kill)
    }

    /// A lifetime can be returned if:
    ///  - it is not present anymore
    ///  - the lifetimes it depends on have changed
    fn lifetimes_to_return(
        &mut self,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> BTreeMap<String, BTreeSet<String>> {
        let mut derived_lifetimes_return: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (lft, old_values) in old_derived_lifetimes.clone() {
            if !new_derived_lifetimes.contains_key(&lft) {
                derived_lifetimes_return.insert(lft.clone(), old_values.clone());
            } else {
                let new_values = new_derived_lifetimes.get(&lft).unwrap().clone();
                if old_values != new_values {
                    derived_lifetimes_return.insert(lft.clone(), old_values.clone());
                }
            }
        }
        derived_lifetimes_return
    }

    /// A lifetime can be taken if:
    ///  - it was newly introduced
    ///  - the lifetimes it depends on have changed
    fn lifetimes_to_take(
        &mut self,
        old_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
        new_derived_lifetimes: &BTreeMap<String, BTreeSet<String>>,
    ) -> BTreeMap<String, BTreeSet<String>> {
        let mut derived_lifetimes_take: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (lft, new_values) in new_derived_lifetimes.clone() {
            if !old_derived_lifetimes.contains_key(&lft) {
                derived_lifetimes_take.insert(lft.clone(), new_values.clone());
            } else {
                let old_values = old_derived_lifetimes.get(&lft).unwrap().clone();
                if old_values != new_values {
                    derived_lifetimes_take.insert(lft.clone(), new_values.clone());
                }
            }
        }
        derived_lifetimes_take
    }

    fn lifetimes_to_end(
        &mut self,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        old_original_lifetimes
            .clone()
            .into_iter()
            .filter(|x| !new_original_lifetimes.contains(x))
            .collect()
    }

    fn lifetimes_to_create(
        &mut self,
        old_original_lifetimes: &BTreeSet<String>,
        new_original_lifetimes: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        new_original_lifetimes
            .clone()
            .into_iter()
            .filter(|x| !old_original_lifetimes.contains(x))
            .collect()
    }

    // TODO: Move this somewhere better
    fn none_permission(&self) -> vir_high::Expression {
        vir_high::Expression::constant_no_pos(
            vir_high::expression::ConstantValue::Int(0),
            vir_high::Type::MPerm,
        )
    }
    // TODO: Move this somewhere better
    fn full_permission(&self) -> vir_high::Expression {
        vir_high::Expression::constant_no_pos(
            vir_high::expression::ConstantValue::Int(1),
            vir_high::Type::MPerm,
        )
    }

    fn encode_lifetime_specifications(
        &mut self,
    ) -> SpannedEncodingResult<(Vec<vir_high::Statement>, Vec<vir_high::Statement>)> {
        let (first_bb, _) = rustc_middle::mir::traversal::reverse_postorder(self.mir)
            .into_iter()
            .next()
            .unwrap();
        let first_location = mir::Location {
            block: first_bb,
            statement_index: 0,
        };

        // construct positive permission amount for inhaling LifetimeTokens
        // let positive_permission_amount = self.encode_per
        let mut preconditions = vec![vir_high::Statement::comment(
            "Lifetime preconditions.".to_string(),
        )];
        let lifetime_token_permission =
            self.fresh_ghost_variable("positive_perm_amount", vir_high::Type::MPerm);
        let none_permission = self.none_permission();
        let full_permission = self.full_permission();
        preconditions.push(
            self.encoder.set_statement_error_ctxt(
                vir_high::Statement::assume_no_pos(
                    vir_high::Expression::binary_op_no_pos(
                        vir_high::BinaryOpKind::GtCmp,
                        lifetime_token_permission.clone().into(),
                        none_permission.into(),
                    ),
                ),
                self.mir.span,
                ErrorCtxt::LifetimeInhale,
                self.def_id,
            )?
        );
        preconditions.push(
            self.encoder.set_statement_error_ctxt(
                vir_high::Statement::assume_no_pos(
                    vir_high::Expression::binary_op_no_pos(
                        vir_high::BinaryOpKind::LtCmp,
                        lifetime_token_permission.clone().into(),
                        full_permission.into(),
                    ),
                ),
                self.mir.span,
                ErrorCtxt::LifetimeInhale,
                self.def_id,
            )?
        );

        // Precondition: Inhale LifetimeTokens
        let lifetimes_to_inhale: BTreeSet<vir_high::ty::LifetimeConst> =
            self.lifetimes_to_inhale()?;
        for lifetime in &lifetimes_to_inhale {
            // TODO: not 1, but some positive permission (Expression?)
            let inhale_statement = self.encode_inhale_lifetime_token(lifetime.clone(), 1)?;
            preconditions.push(inhale_statement);
        }

        // Postcondition: Exhale (inhaled) LifetimeTokens
        let mut postconditions = vec![vir_high::Statement::comment(
            "Lifetime postconditions.".to_string(),
        )];
        for lifetime in lifetimes_to_inhale {
            let exhale_statement = self.encode_exhale_lifetime_token(lifetime)?;
            postconditions.push(exhale_statement);
        }

        // Precondition: Assume opaque lifetime conditions
        let opaque_conditions: BTreeMap<String, BTreeSet<String>> =
            self.lifetimes.get_opaque_lifetimes_with_inclusions_names();
        let mut opaque_lifetimes: BTreeSet<vir_high::ty::LifetimeConst> = BTreeSet::new();
        for (lifetime, condition) in &opaque_conditions {
            let lifetime_const = vir_high::ty::LifetimeConst {
                name: lifetime.to_string(),
            };
            opaque_lifetimes.insert(lifetime_const.clone());
            let assume_statement = self.encoder.set_statement_error_ctxt(
                vir_high::Statement::lifetime_included_no_pos(
                    false, // assume, not assert
                    lifetime_const,
                    condition
                        .iter()
                        .map(|lft| vir_high::ty::LifetimeConst { name: lft.clone() })
                        .collect(),
                ),
                self.mir.span,
                ErrorCtxt::LifetimeEncoding,
                self.def_id,
            )?;
            preconditions.push(assume_statement);
        }

        // Precondition: LifetimeTake for subset lifetimes
        let lifetime_subsets: BTreeSet<(String, String)> = self
            .lifetimes
            .get_subset_base_at_start(first_location)
            .iter()
            .map(|(r1, r2)| (r1.to_text(), r2.to_text()))
            .collect();
        let identical_lifetimes = self.identical_lifetimes(
            opaque_conditions.keys().cloned().collect(),
            lifetime_subsets,
        );
        for (new_lifetime, existing_lifetime) in identical_lifetimes {
            let encoded_target = self.encode_lft_variable(new_lifetime)?;
            let encoded_source = self.encode_lft_variable(existing_lifetime)?;
            let statement = self.encoder.set_statement_error_ctxt(
                vir_high::Statement::lifetime_take_no_pos(
                    encoded_target,
                    vec![encoded_source],
                    self.rd_perm,
                ),
                self.mir.span,
                ErrorCtxt::LifetimeEncoding,
                self.def_id,
            )?;
            preconditions.push(statement);
        }
        Ok((preconditions, postconditions))
    }

    fn get_lifetime_name(&mut self, expression: vir_high::Expression) -> Option<String> {
        if let vir_high::Expression::Local(vir_high::Local {
            variable:
                vir_high::VariableDecl {
                    name: _,
                    ty: vir_high::ty::Type::Reference(vir_high::ty::Reference { lifetime, .. }),
                },
            ..
        }) = expression
        {
            return Some(lifetime.name);
        }
        None
    }

    fn identical_lifetimes(
        &mut self,
        existing_lifetimes: BTreeSet<String>,
        relations: BTreeSet<(String, String)>,
    ) -> BTreeMap<String, String> {
        let unique_lifetimes: BTreeSet<String> = relations
            .iter()
            .flat_map(|(x, y)| [x, y])
            .cloned()
            .collect();
        let n = unique_lifetimes.len(); // rather naive upper bound
        let mut lft_enumarate: BTreeMap<String, usize> = BTreeMap::new();
        let mut lft_enumarate_rev: BTreeMap<usize, String> = BTreeMap::new();

        for (i, e) in unique_lifetimes.iter().enumerate() {
            lft_enumarate.insert(e.to_string(), i);
            lft_enumarate_rev.insert(i, e.to_string());
        }

        let graph = {
            let mut g = Graph::new(n);
            for (k, v) in relations {
                g.add_edge(
                    *lft_enumarate.get(&k[..]).unwrap(),
                    *lft_enumarate.get(&v[..]).unwrap(),
                );
            }
            g
        };

        // compute strongly connected components
        let mut identical_lifetimes: BTreeSet<BTreeSet<String>> = BTreeSet::new();
        for component in Tarjan::walk(&graph) {
            identical_lifetimes.insert(
                component
                    .iter()
                    .map(|x| lft_enumarate_rev.get(x).unwrap())
                    .cloned()
                    .collect(),
            );
        }

        // put data in correct shape
        let mut identical_lifetimes_map: BTreeMap<String, String> = BTreeMap::new();
        for component in identical_lifetimes {
            dbg!(&component);
            let existing_component_lifetims: BTreeSet<String> = component
                .iter()
                .cloned()
                .filter(|lft| existing_lifetimes.contains(&lft[..]))
                .collect();
            // TODO: typo!
            let non_existing_component_lifetimes: BTreeSet<String> = component
                .iter()
                .cloned()
                .filter(|lft| !existing_lifetimes.contains(&lft[..]))
                .collect();
            for lifetime in non_existing_component_lifetimes {
                let identical_existing_lifetime = existing_component_lifetims.iter().next();
                if let Some(identical_existing_lifetime) = identical_existing_lifetime {
                    identical_lifetimes_map.insert(lifetime, identical_existing_lifetime.clone());
                }
            }
        }
        identical_lifetimes_map
    }

    fn lifetimes_to_inhale(
        &mut self,
    ) -> SpannedEncodingResult<BTreeSet<vir_high::ty::LifetimeConst>> {
        Ok(self
            .lifetimes
            .get_opaque_lifetimes_with_inclusions_names()
            .keys()
            .map(|x| vir_high::ty::LifetimeConst {
                name: x.to_string(),
            })
            .collect())
    }

    fn lifetimes_to_exhale_inhale_function(
        &mut self,
    ) -> SpannedEncodingResult<BTreeSet<vir_high::ty::LifetimeConst>> {
        Ok(BTreeSet::new())
    }

    fn encode_inhale_lifetime_token(
        &mut self,
        lifetime_const: vir_high::ty::LifetimeConst,
        rd_perm: u32,
    ) -> SpannedEncodingResult<vir_high::Statement> {
        self.encoder.set_statement_error_ctxt(
            vir_high::Statement::inhale_no_pos(vir_high::Predicate::lifetime_token_no_pos(
                lifetime_const,
                rd_perm,
            )),
            self.mir.span,
            ErrorCtxt::LifetimeInhale,
            self.def_id,
        )
    }

    fn encode_exhale_lifetime_token(
        &mut self,
        lifetime_const: vir_high::ty::LifetimeConst,
    ) -> SpannedEncodingResult<vir_high::Statement> {
        self.encoder.set_statement_error_ctxt(
            vir_high::Statement::exhale_no_pos(vir_high::Predicate::lifetime_token_no_pos(
                lifetime_const,
                self.rd_perm,
            )),
            self.mir.span,
            ErrorCtxt::LifetimeExhale,
            self.def_id,
        )
    }
}
