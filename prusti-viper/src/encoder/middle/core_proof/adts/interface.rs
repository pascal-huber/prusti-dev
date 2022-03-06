use std::borrow::Cow;

use super::types::{
    alternative_struct_destructor_name, constant_destructor_name, constructor_call,
    constructor_name, destructor_call, enum_variant_destructor_call, AdtConstructorKind,
};
use crate::encoder::{
    errors::SpannedEncodingResult,
    middle::core_proof::lowerer::{DomainsLowererInterface, Lowerer},
};
use rustc_hash::FxHashSet;
use vir_crate::{
    common::expression::{ExpressionIterator, QuantifierHelpers},
    low::{self as vir_low},
};

#[derive(Default)]
pub(in super::super) struct AdtsState {
    /// Registered default constructors for a given domain.
    ///
    /// These are typically used in cases where the ADT has only a single
    /// constructor.
    main_constructors: FxHashSet<vir_low::ty::Domain>,
    /// Registered variant constructors for a given domain.
    ///
    /// These are typically used in cases where an ADT has multiple variants.
    variant_constructors: FxHashSet<(vir_low::ty::Domain, String)>,
}

const NO_VARIANT_NAME: &str = "";

pub(in super::super) trait AdtsInterface {
    fn adt_constructor_main_name(&mut self, domain_name: &str) -> SpannedEncodingResult<String> {
        self.adt_constructor_variant_name(domain_name, NO_VARIANT_NAME)
    }
    fn adt_constructor_variant_name(
        &mut self,
        domain_name: &str,
        variant_name: &str,
    ) -> SpannedEncodingResult<String>;
    fn adt_destructor_main_name(
        &mut self,
        domain_name: &str,
        parameter_name: &str,
    ) -> SpannedEncodingResult<String> {
        self.adt_destructor_variant_name(domain_name, NO_VARIANT_NAME, parameter_name)
    }
    fn adt_destructor_variant_name(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        parameter_name: &str,
    ) -> SpannedEncodingResult<String>;
    fn adt_constructor_main_call(
        &mut self,
        domain_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        self.adt_constructor_variant_call(domain_name, NO_VARIANT_NAME, arguments)
    }
    fn adt_constructor_variant_call(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    fn adt_destructor_main_call(
        &mut self,
        domain_name: &str,
        parameter_name: &str,
        parameter_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        self.adt_destructor_variant_call(
            domain_name,
            NO_VARIANT_NAME,
            parameter_name,
            parameter_type,
            argument,
        )
    }
    fn adt_destructor_variant_call(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        parameter_name: &str,
        parameter_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    /// Register the main constructor and derive injectivity axioms for it. If
    /// `top_down_injectivity_guard` is not `None`, when deriving the top-down
    /// injectivity axiom, it is called with the quantified variable and its
    /// result is used to guard the injectivity property. This is intended to be
    /// used by the snapshot encoder to supply call to the validity function.
    fn adt_register_main_constructor<F>(
        &mut self,
        domain_name: &str,
        parameters: Vec<vir_low::VariableDecl>,
        top_down_injectivity_guard: Option<F>,
    ) -> SpannedEncodingResult<()>
    where
        F: for<'a> FnOnce(
            &'a str,
            &'a vir_low::VariableDecl,
        ) -> SpannedEncodingResult<vir_low::Expression>;
    /// Register the variant constructor and derive injectivity axioms for it.
    /// If `top_down_injectivity_guard` is not `None`, when deriving the
    /// top-down injectivity axiom, it is called with the quantified variable
    /// and its result is used to guard the injectivity property. This is
    /// intended to be used by the snapshot encoder to supply call to the
    /// validity function.
    fn adt_register_variant_constructor<F>(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        parameters: Vec<vir_low::VariableDecl>,
        top_down_injectivity_guard: Option<F>,
    ) -> SpannedEncodingResult<()>
    where
        F: for<'a> FnOnce(
            &'a str,
            &'a vir_low::VariableDecl,
        ) -> SpannedEncodingResult<vir_low::Expression>;

    fn snapshot_constructor_constant_call(
        &mut self,
        domain_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    fn snapshot_destructor_constant_name(
        &mut self,
        domain_name: &str,
    ) -> SpannedEncodingResult<String>;
    fn snapshot_constructor_struct_name(
        &mut self,
        domain_name: &str,
    ) -> SpannedEncodingResult<String>;
    fn snapshot_destructor_struct_call(
        &mut self,
        domain_name: &str,
        field_name: &str,
        field_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    fn snapshot_constructor_struct_alternative_name(
        &mut self,
        domain_name: &str,
        variant: &str,
    ) -> SpannedEncodingResult<String>;
    // fn adt_destructor_struct_alternative_name(
    //     &mut self,
    //     domain_name: &str,
    //     variant: &str,
    //     field_name: &str,
    // ) -> SpannedEncodingResult<String>;
    fn snapshot_destructor_enum_variant_call(
        &mut self,
        domain_name: &str,
        variant: &str,
        variant_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression>;
}

impl<'p, 'v: 'p, 'tcx: 'v> AdtsInterface for Lowerer<'p, 'v, 'tcx> {
    fn adt_constructor_variant_name(
        &mut self,
        domain_name: &str,
        variant_name: &str,
    ) -> SpannedEncodingResult<String> {
        Ok(format!("constructor${}${}", domain_name, variant_name))
    }
    fn adt_destructor_variant_name(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        parameter_name: &str,
    ) -> SpannedEncodingResult<String> {
        Ok(format!(
            "destructor${}${}${}",
            domain_name, variant_name, parameter_name
        ))
    }
    fn adt_constructor_variant_call(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        Ok(vir_low::Expression::domain_function_call(
            domain_name.to_string(),
            self.adt_constructor_variant_name(domain_name, variant_name)?,
            arguments,
            vir_low::Type::domain(domain_name.to_string()),
        ))
    }
    fn adt_destructor_variant_call(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        parameter_name: &str,
        parameter_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        Ok(vir_low::Expression::domain_function_call(
            domain_name.to_string(),
            self.adt_destructor_variant_name(domain_name, variant_name, parameter_name)?,
            vec![argument],
            parameter_type,
        ))
    }
    fn adt_register_main_constructor<F>(
        &mut self,
        domain_name: &str,
        parameters: Vec<vir_low::VariableDecl>,
        top_down_injectivity_guard: Option<F>,
    ) -> SpannedEncodingResult<()>
    where
        F: for<'a> FnOnce(
            &'a str,
            &'a vir_low::VariableDecl,
        ) -> SpannedEncodingResult<vir_low::Expression>,
    {
        assert!(self
            .adts_state
            .main_constructors
            .insert(vir_low::ty::Domain::new(domain_name)));
        self.adt_register_variant_constructor(
            domain_name,
            "",
            parameters,
            top_down_injectivity_guard,
        )
    }
    fn adt_register_variant_constructor<F>(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        parameters: Vec<vir_low::VariableDecl>,
        top_down_injectivity_guard: Option<F>,
    ) -> SpannedEncodingResult<()>
    where
        F: for<'a> FnOnce(
            &'a str,
            &'a vir_low::VariableDecl,
        ) -> SpannedEncodingResult<vir_low::Expression>,
    {
        assert!(self.adts_state.variant_constructors.insert((
            vir_low::ty::Domain::new(domain_name),
            variant_name.to_string()
        )));
        let ty = vir_low::Type::domain(domain_name.to_string());

        // Constructor.
        let constructor_name = self.adt_constructor_variant_name(domain_name, variant_name)?;
        // let function = vir_low::DomainFunctionDecl {
        //     name: constructor_name.clone(),
        //     parameters: parameters.clone(),
        //     return_type: ty.clone(),
        // };
        self.declare_domain_function(
            domain_name,
            Cow::Borrowed(&constructor_name),
            Cow::Borrowed(&parameters),
            Cow::Borrowed(&ty),
        )?;

        // Destructors.
        let value = vir_low::VariableDecl::new("value", ty.clone());
        for parameter in &parameters {
            let destructor_name =
                self.adt_destructor_variant_name(domain_name, variant_name, &parameter.name)?;
            // let function = vir_low::DomainFunctionDecl {
            //     name: destructor_name,
            //     parameters: vec![value.clone()],
            //     return_type: parameter.ty.clone(),
            // };
            // self.insert_domain_function(domain_name, function)?;
            self.declare_domain_function(
                domain_name,
                Cow::Owned(destructor_name),
                Cow::Owned(vec![value.clone()]),
                Cow::Borrowed(&parameter.ty),
            )?;
        }

        // Injectivity axioms.
        if parameters.is_empty() {
            // No need to generate injectivity axioms if the constructor has no parameters.
            return Ok(());
        }

        use vir_low::macros::*;
        // Bottom-up injectivity axiom.
        {
            let mut triggers = Vec::new();
            let mut conjuncts = Vec::new();
            let constructor_call = self.adt_constructor_variant_call(
                domain_name,
                variant_name,
                parameters
                    .iter()
                    .map(|argument| argument.clone().into())
                    .collect(),
            )?;
            for parameter in &parameters {
                let destructor_call = self.adt_destructor_variant_call(
                    domain_name,
                    variant_name,
                    &parameter.name,
                    parameter.ty.clone(),
                    constructor_call.clone(),
                )?;
                triggers.push(vir_low::Trigger::new(vec![destructor_call.clone()]));
                conjuncts.push(expr! { [destructor_call] == parameter });
            }
            let body = vir_low::Expression::forall(
                parameters.clone(),
                triggers,
                conjuncts.into_iter().conjoin(),
            );
            let axiom = vir_low::DomainAxiomDecl {
                name: format!("{}$bottom_up_injectivity_axiom", constructor_name),
                body,
            };
            self.declare_axiom(&domain_name, axiom)?;
        }

        // Top-down injectivity axiom.
        {
            var_decls! { value: {ty} };
            let guard = if let Some(guard_constructor) = top_down_injectivity_guard {
                Some(guard_constructor(domain_name, &value)?)
            } else {
                None
            };
            let mut triggers = Vec::new();
            let mut arguments = Vec::new();
            for parameter in &parameters {
                let destructor_call = self.adt_destructor_variant_call(
                    domain_name,
                    variant_name,
                    &parameter.name,
                    parameter.ty.clone(),
                    value.clone().into(),
                )?;
                if let Some(guard) = &guard {
                    triggers.push(vir_low::Trigger::new(vec![
                        guard.clone(),
                        destructor_call.clone(),
                    ]));
                } else {
                    unimplemented!("figure out what triggers to choose to avoid matching loop!");
                }
                arguments.push(destructor_call);
            }
            let constructor_call =
                self.adt_constructor_variant_call(domain_name, variant_name, arguments)?;
            let equality = expr! { value == [constructor_call] };
            let forall_body = if let Some(guard) = guard {
                expr! { [guard] ==> [equality] }
            } else {
                equality
            };
            let axiom = vir_low::DomainAxiomDecl {
                name: format!("{}$top_down_injectivity_axiom", constructor_name),
                body: vir_low::Expression::forall(vec![value.into()], triggers, forall_body),
            };
            self.declare_axiom(&domain_name, axiom)?;
        }

        Ok(())
    }

    // TODO: Move to SnapshotADTs
    fn snapshot_constructor_constant_call(
        &mut self,
        domain_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        let function_name = self.adt_constructor_main_name(domain_name)?;
        let return_type = vir_low::Type::domain(domain_name.to_string());
        Ok(vir_low::Expression::domain_function_call(
            domain_name,
            function_name,
            arguments,
            return_type,
        ))
        // Ok(constructor_call(
        //     domain_name,
        //     &AdtConstructorKind::Constant,
        //     arguments,
        // ))
    }
    fn snapshot_destructor_constant_name(
        &mut self,
        domain_name: &str,
    ) -> SpannedEncodingResult<String> {
        self.adt_destructor_main_name(domain_name, "value")
    }
    fn snapshot_constructor_struct_name(
        &mut self,
        domain_name: &str,
    ) -> SpannedEncodingResult<String> {
        self.adt_constructor_main_name(domain_name)
        // Ok(constructor_name(domain_name, &AdtConstructorKind::Struct))
    }
    fn snapshot_destructor_struct_call(
        &mut self,
        domain_name: &str,
        field_name: &str,
        field_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        self.adt_destructor_main_call(domain_name, field_name, field_type, argument)
        // Ok(destructor_call(
        //     domain_name,
        //     &AdtConstructorKind::Struct,
        //     field_name,
        //     field_type,
        //     argument,
        // ))
    }
    fn snapshot_constructor_struct_alternative_name(
        &mut self,
        domain_name: &str,
        variant_name: &str,
    ) -> SpannedEncodingResult<String> {
        self.adt_constructor_variant_name(domain_name, variant_name)
        // Ok(constructor_name(
        //     domain_name,
        //     &AdtConstructorKind::AlternativeStruct {
        //         name: variant.to_string(),
        //     },
        // ))
    }
    // fn adt_destructor_struct_alternative_name(
    //     &mut self,
    //     domain_name: &str,
    //     variant: &str,
    //     field_name: &str,
    // ) -> SpannedEncodingResult<String> {
    //     Ok(alternative_struct_destructor_name(
    //         domain_name,
    //         variant,
    //         field_name,
    //     ))
    // }
    fn snapshot_destructor_enum_variant_call(
        &mut self,
        domain_name: &str,
        variant_name: &str,
        variant_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        unimplemented!();
        // Ok(enum_variant_destructor_call(
        //     domain_name,
        //     variant,
        //     variant_type,
        //     argument,
        // ))
    }
}
