use crate::encoder::{
    errors::SpannedEncodingResult,
    middle::core_proof::{
        lowerer::{DomainsLowererInterface, Lowerer, PredicatesLowererInterface},
        snapshots::IntoProcedureSnapshot,
    },
};
use vir_crate::{
    common::expression::{BinaryOperationHelpers, QuantifierHelpers},
    low as vir_low, middle as vir_mid,
};

#[derive(Default)]
pub(in super::super) struct LifetimesState {
    is_lifetime_token_encoded: bool,
}

pub(in super::super) trait LifetimesInterface {
    fn lifetime_domain_name(&self) -> SpannedEncodingResult<String>;
    fn lifetime_type(&mut self) -> SpannedEncodingResult<vir_low::Type>;
    fn encode_lifetime_intersect(&mut self) -> SpannedEncodingResult<vir_low::Expression>;
    fn encode_lifetime_included(&mut self) -> SpannedEncodingResult<vir_low::Expression>;
    fn encode_lifetime_included_intersect_axiom(&mut self) -> SpannedEncodingResult<()>;
    fn encode_lifetime_token_predicate(&mut self) -> SpannedEncodingResult<()>;
    fn encode_lifetime_const_into_variable(
        &mut self,
        lifetime: vir_mid::ty::LifetimeConst,
    ) -> SpannedEncodingResult<vir_low::VariableDecl>;
    fn extract_lifetime_variables(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>>;
    fn extract_lifetime_variables_as_expr(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>>;
}

impl<'p, 'v: 'p, 'tcx: 'v> LifetimesInterface for Lowerer<'p, 'v, 'tcx> {
    fn lifetime_domain_name(&self) -> SpannedEncodingResult<String> {
        Ok("Lifetime".to_string())
    }

    fn lifetime_type(&mut self) -> SpannedEncodingResult<vir_low::Type> {
        self.domain_type(self.lifetime_domain_name()?)
    }

    fn encode_lifetime_intersect(&mut self) -> SpannedEncodingResult<vir_low::Expression> {
        let ty = self.domain_type("Lifetime")?;
        // TODO: why VariableDecl if name is ignored anyways??
        let arguments: Vec<vir_low::Expression> = vec![
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new(
                "lft_1".to_string(),
                ty.clone(),
            )),
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new(
                "lft_2".to_string(),
                ty.clone(),
            )),
        ];
        self.create_domain_func_app("Lifetime", "intersect$", arguments, ty, Default::default())
    }

    fn encode_lifetime_included(&mut self) -> SpannedEncodingResult<vir_low::Expression> {
        let ty = self.domain_type("Lifetime")?;
        // TODO: why VariableDecl if name is ignored anyways??
        let arguments: Vec<vir_low::Expression> = vec![
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new(
                "lft_1".to_string(),
                ty.clone(),
            )),
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new("lft_2".to_string(), ty)),
        ];
        self.create_domain_func_app(
            "Lifetime",
            "included$",
            arguments,
            vir_low::ty::Type::Bool,
            Default::default(),
        )
    }

    fn encode_lifetime_included_intersect_axiom(&mut self) -> SpannedEncodingResult<()> {
        let ty = self.domain_type("Lifetime")?;
        use vir_low::macros::*;
        let variables = vars! { lft_1 : Lifetime, lft_2 : Lifetime };
        let mut trigger_expressions = vec![];

        // Arguments for Triggers and Body
        let arguments_all_lifetimes = vec![
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new(
                "lft_1".to_string(),
                ty.clone(),
            )),
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new(
                "lft_2".to_string(),
                ty.clone(),
            )),
        ];

        let arguments_1: Vec<vir_low::Expression> = vec![
            self.create_domain_func_app(
                "Lifetime",
                "itersect$",
                arguments_all_lifetimes.clone(),
                ty.clone(),
                Default::default(),
            )?,
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new(
                "lft_1".to_string(),
                ty.clone(),
            )),
        ];
        let arguments_2: Vec<vir_low::Expression> = vec![
            self.create_domain_func_app(
                "Lifetime",
                "itersect$",
                arguments_all_lifetimes,
                ty.clone(),
                Default::default(),
            )?,
            vir_low::Expression::local_no_pos(vir_low::VariableDecl::new("lft_2".to_string(), ty)),
        ];

        // Triggers
        trigger_expressions.push(self.create_domain_func_app(
            "Lifetime",
            "included$",
            arguments_1.clone(),
            vir_low::ty::Type::Bool,
            Default::default(),
        )?);
        trigger_expressions.push(self.create_domain_func_app(
            "Lifetime",
            "included$",
            arguments_2.clone(),
            vir_low::ty::Type::Bool,
            Default::default(),
        )?);
        let triggers = vec![vir_low::Trigger {
            terms: trigger_expressions,
        }];

        let quantifier_body = BinaryOperationHelpers::and(
            self.create_domain_func_app(
                "Lifetime",
                "included$",
                arguments_1,
                vir_low::ty::Type::Bool,
                Default::default(),
            )?,
            self.create_domain_func_app(
                "Lifetime",
                "included$",
                arguments_2,
                vir_low::ty::Type::Bool,
                Default::default(),
            )?,
        );

        let axiom = vir_low::DomainAxiomDecl {
            name: String::from("included_intersect"),
            body: QuantifierHelpers::forall(variables, triggers, quantifier_body),
        };
        self.declare_axiom("Lifetime", axiom)?;
        Ok(())
    }

    fn encode_lifetime_token_predicate(&mut self) -> SpannedEncodingResult<()> {
        if !self.lifetimes_state.is_lifetime_token_encoded {
            self.lifetimes_state.is_lifetime_token_encoded = true;
            let predicate = vir_low::PredicateDecl::new(
                "LifetimeToken",
                vec![vir_low::VariableDecl::new(
                    "lifetime",
                    self.lifetime_type()?,
                )],
                None,
            );
            self.declare_predicate(predicate)?;
            let predicate = vir_low::PredicateDecl::new(
                "DeadLifetimeToken",
                vec![vir_low::VariableDecl::new(
                    "lifetime",
                    self.lifetime_type()?,
                )],
                None,
            );
            self.declare_predicate(predicate)?;
        }
        Ok(())
    }

    fn encode_lifetime_const_into_variable(
        &mut self,
        lifetime: vir_mid::ty::LifetimeConst,
    ) -> SpannedEncodingResult<vir_low::VariableDecl> {
        let lifetime_variable = vir_mid::VariableDecl::new(lifetime.name, vir_mid::Type::Lifetime);
        lifetime_variable.to_procedure_snapshot(self)
    }

    fn extract_lifetime_variables(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::VariableDecl>> {
        let mut lifetimes = Vec::new();
        for lifetime in ty.get_lifetimes() {
            lifetimes.push(self.encode_lifetime_const_into_variable(lifetime)?);
        }
        Ok(lifetimes)
    }

    fn extract_lifetime_variables_as_expr(
        &mut self,
        ty: &vir_mid::Type,
    ) -> SpannedEncodingResult<Vec<vir_low::Expression>> {
        Ok(self
            .extract_lifetime_variables(ty)?
            .into_iter()
            .map(|lifetime| lifetime.into())
            .collect())
    }
}
