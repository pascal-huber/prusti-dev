use crate::encoder::{
    errors::SpannedEncodingResult,
    middle::core_proof::{
        lowerer::{DomainsLowererInterface, Lowerer, PredicatesLowererInterface},
        snapshots::IntoProcedureSnapshot,
    },
};
use vir_crate::{
    low as vir_low,
    middle as vir_mid,
    common::expression::{BinaryOperationHelpers, QuantifierHelpers},
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
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_1".to_string(), ty.clone())
            ),
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_2".to_string(), ty.clone())
            ),
        ];
        self.create_domain_func_app(
            "Lifetime",
            "intersect$",
            arguments,
            ty,
            Default::default(),
        )
    }

    fn encode_lifetime_included(&mut self) -> SpannedEncodingResult<vir_low::Expression> {
        let ty = self.domain_type("Lifetime")?;
        // TODO: why VariableDecl if name is ignored anyways??
        let arguments: Vec<vir_low::Expression> = vec![
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_1".to_string(), ty.clone())
            ),
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_2".to_string(), ty.clone())
            ),
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

        // axiom included_intersect {
    //     forall lft1: Lifetime, lft2: Lifetime ::
    //         {included(intersect(lft1, lft2), lft1)}{included(intersect(lft1, lft2), lft2)}
    //         included(intersect(lft1, lft2), lft1) &&
    //         included(intersect(lft1, lft2), lft2)
    // }


        // ( forall( $( $var_name:ident : $var_type:tt ),* :: $( raw_code { $( $statement:stmt; )+ } )? [$( { $( $trigger:tt ),+ } ),*] $($body: tt)+ ) ) => {
        // {
        // $( let $var_name = $crate::low::macros::var! { $var_name: $var_type }; )*
        // $( $( $statement; )+ )?
        // $crate::low::ast::expression::Expression::quantifier_no_pos(
        // $crate::low::ast::expression::QuantifierKind::ForAll,
        // $crate::low::macros::vars!{ $( $var_name : $var_type ),* },
        // vec![
        // $(
        // $crate::low::ast::expression::Trigger::new(
        // vec![
        // $( $crate::low::macros::expr!($trigger) ),+
        // ]
        // )
        // ),*
        // ],
        // $crate::low::macros::expr!($($body)+)
        // )
        // }
        // },


        use vir_low::{macros::*};
        let variables = vars!{ lft_1 : Lifetime, lft_2 : Lifetime };


        let mut trigger_expressions = vec![];

        // Arguments for Triggers and Body
        let arguments_intersects = vec![
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_1".to_string(), ty.clone())
            ),
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_2".to_string(), ty.clone())
            ),
        ];
        let arguments_1: Vec<vir_low::Expression> = vec![
            self.create_domain_func_app(
                "Lifetime",
                "itersect$",
                arguments_intersects.clone(),
                ty.clone(),
                Default::default(),
            )?,
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_1".to_string(), ty.clone())
            ),
        ];
        let arguments_2: Vec<vir_low::Expression> = vec![
            self.create_domain_func_app(
                "Lifetime",
                "itersect$",
                arguments_intersects.clone(),
                ty.clone(),
                Default::default(),
            )?,
            vir_low::Expression::local_no_pos(
                vir_low::VariableDecl::new("lft_2".to_string(), ty.clone())
            ),
        ];

        // Triggers
        trigger_expressions.push(
            self.create_domain_func_app(
                "Lifetime",
                "included$",
                arguments_1.clone(),
                vir_low::ty::Type::Bool,
                Default::default(),
            )?
        );
        trigger_expressions.push(
            self.create_domain_func_app(
                "Lifetime",
                "included$",
                arguments_2.clone(),
                vir_low::ty::Type::Bool,
                Default::default(),
            )?
        );
        let triggers = vec![vir_low::Trigger {
            terms: trigger_expressions,
        }];

        // let quantifier_body = vir_low::Expression::equals(
        //         expr!{true},
        //         expr!{true}
        // ) // just for test... // conjuncts.into_iter().conjoin(), // Expression
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
            )?
        );


        let body = QuantifierHelpers::forall(
            variables, // vec<Expression::BoundedVariableDecl>
            triggers, // Vec<Expresion::Trigger>
            quantifier_body,
        );
        let axiom = vir_low::DomainAxiomDecl {
            name: String::from("included_intersect"),
            body,
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
