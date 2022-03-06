use vir_crate::{
    common::expression::{ExpressionIterator, QuantifierHelpers},
    low::{self as vir_low},
};

// pub(super) const CONSTANT_VARIANT: &str = "constant$";
// pub(super) const CONSTANT_PARAMETER: &str = "constant";
// pub(super) const VARIANT_PARAMETER: &str = "variant";
// pub(super) const BASE_VARIANT: &str = "base$";

pub(in super::super) enum AdtConstructorKind {
    /// A constructor that constructs the type from a constant.
    Constant,
    /// A constructor that constructs the type from given parameters.
    Struct,
    /// An alternative constructor for the struct type.
    AlternativeStruct { name: String },
    /// A constructor that creates a specific variant of a enum.
    EnumVariant { name: String },
}

pub(in super::super) struct AdtConstructor {
    kind: AdtConstructorKind,
    parameters: Vec<vir_low::VariableDecl>,
}

impl AdtConstructor {
    // pub(in super::super) fn constant(parameter_type: vir_crate::low::Type) -> AdtConstructor {
    //     Self {
    //         kind: AdtConstructorKind::Constant,
    //         parameters: vec![vir_low::VariableDecl::new("constant", parameter_type)],
    //     }
    // }

    // pub(in super::super) fn struct_main(parameters: Vec<vir_low::VariableDecl>) -> AdtConstructor {
    //     Self {
    //         kind: AdtConstructorKind::Struct,
    //         parameters,
    //     }
    // }

    // pub(in super::super) fn struct_alternative(
    //     name: String,
    //     parameters: Vec<vir_low::VariableDecl>,
    // ) -> AdtConstructor {
    //     Self {
    //         kind: AdtConstructorKind::AlternativeStruct { name },
    //         parameters,
    //     }
    // }

    // pub(in super::super) fn enum_variant(
    //     name: String,
    //     variant_type: vir_crate::low::Type,
    // ) -> AdtConstructor {
    //     Self {
    //         kind: AdtConstructorKind::EnumVariant { name },
    //         parameters: vec![vir_low::VariableDecl::new("variant", variant_type)],
    //     }
    // }

    // // pub(in super::super) fn get_variant(&self) -> &str {
    // //     &self.variant
    // // }

    // pub(in super::super) fn get_parameters(&self) -> &[vir_low::VariableDecl] {
    //     &self.parameters
    // }

    pub(in super::super) fn parameter_destructor_name(
        &self,
        domain_name: &str,
        field_name: &str,
    ) -> String {
        parameter_destructor_name(domain_name, &self.kind, field_name)
    }

    pub(in super::super) fn create_constructor_function(
        &self,
        domain_name: &str,
    ) -> vir_low::DomainFunctionDecl {
        vir_low::DomainFunctionDecl {
            name: constructor_name(domain_name, &self.kind),
            parameters: self.parameters.clone(),
            return_type: constructor_return_type(domain_name),
        }
    }

    pub(in super::super) fn constructor_call(
        &self,
        domain_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> vir_low::Expression {
        constructor_call(domain_name, &self.kind, arguments)
    }

    pub(in super::super) fn default_constructor_call(
        &self,
        domain_name: &str,
    ) -> vir_low::Expression {
        self.constructor_call(
            domain_name,
            self.parameters
                .iter()
                .map(|argument| argument.clone().into())
                .collect(),
        )
    }

    pub(in super::super) fn constructor_name(&self, domain_name: &str) -> String {
        constructor_name(domain_name, &self.kind)
    }

    pub(in super::super) fn destructor_call(
        &self,
        domain_name: &str,
        field_name: &str,
        field_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> vir_low::Expression {
        destructor_call(domain_name, &self.kind, field_name, field_type, argument)
    }

    pub(in super::super) fn create_destructor_functions(
        &self,
        domain_name: &str,
    ) -> Vec<vir_low::DomainFunctionDecl> {
        let value =
            vir_low::VariableDecl::new("value", vir_low::Type::domain(domain_name.to_string()));
        self.parameters
            .iter()
            .map(|parameter| vir_low::DomainFunctionDecl {
                name: self.parameter_destructor_name(domain_name, &parameter.name),
                parameters: vec![value.clone()],
                return_type: parameter.ty.clone(),
            })
            .collect()
    }

    pub(in super::super) fn create_injectivity_axioms(
        &self,
        domain_name: &str,
    ) -> Vec<vir_low::DomainAxiomDecl> {
        let constructor_call = self.default_constructor_call(domain_name);
        let axioms = if self.parameters.is_empty() {
            vec![]
        } else {
            use vir_low::macros::*;
            let mut triggers = Vec::new();
            let mut conjuncts = Vec::new();
            for field in &self.parameters {
                let destructor_call = destructor_call(
                    domain_name,
                    &self.kind,
                    &field.name,
                    field.ty.clone(),
                    constructor_call.clone(),
                );
                triggers.push(vir_low::Trigger::new(vec![destructor_call.clone()]));
                conjuncts.push(expr! { [destructor_call] == field });
            }
            let body = vir_low::Expression::forall(
                self.parameters.clone(),
                triggers,
                conjuncts.into_iter().conjoin(),
            );
            vec![vir_low::DomainAxiomDecl {
                name: format!(
                    "{}$injectivity_axiom",
                    constructor_name(domain_name, &self.kind)
                ),
                body,
            }]
        };
        axioms
    }
}

fn constructor_return_type(domain_name: &str) -> vir_low::Type {
    vir_low::Type::domain(domain_name.to_string())
}

pub(super) fn constructor_call(
    domain_name: &str,
    kind: &AdtConstructorKind,
    arguments: Vec<vir_low::Expression>,
) -> vir_low::Expression {
    vir_low::Expression::domain_function_call(
        domain_name.to_string(),
        constructor_name(domain_name, kind),
        arguments,
        constructor_return_type(domain_name),
    )
}

pub(super) fn destructor_call(
    domain_name: &str,
    kind: &AdtConstructorKind,
    field_name: &str,
    field_type: vir_low::Type,
    argument: vir_low::Expression,
) -> vir_low::Expression {
    vir_low::Expression::domain_function_call(
        domain_name.to_string(),
        parameter_destructor_name(domain_name, kind, field_name),
        vec![argument],
        field_type,
    )
}

pub(super) fn enum_variant_destructor_call(
    domain_name: &str,
    variant: &str,
    variant_type: vir_low::Type,
    argument: vir_low::Expression,
) -> vir_low::Expression {
    vir_low::Expression::domain_function_call(
        domain_name.to_string(),
        enum_variant_destructor_name(domain_name, variant),
        vec![argument],
        variant_type,
    )
}

pub(super) fn constructor_name(domain_name: &str, kind: &AdtConstructorKind) -> String {
    match kind {
        AdtConstructorKind::Constant => format!("constructor${}", domain_name),
        AdtConstructorKind::Struct => format!("constructor${}", domain_name),
        AdtConstructorKind::AlternativeStruct { name } => {
            format!("constructor${}${}", domain_name, name)
        }
        AdtConstructorKind::EnumVariant { name } => format!("constructor${}${}", domain_name, name),
    }
}

pub(super) fn parameter_destructor_name(
    domain_name: &str,
    kind: &AdtConstructorKind,
    parameter_name: &str,
) -> String {
    match kind {
        AdtConstructorKind::Constant => constant_destructor_name(domain_name),
        AdtConstructorKind::Struct => format!("field${}${}", domain_name, parameter_name),
        AdtConstructorKind::AlternativeStruct { name } => {
            alternative_struct_destructor_name(domain_name, name, parameter_name)
        }
        AdtConstructorKind::EnumVariant { name } => enum_variant_destructor_name(domain_name, name),
    }
}

pub(super) fn constant_destructor_name(domain_name: &str) -> String {
    format!("destructor${}", domain_name)
}

pub(super) fn alternative_struct_destructor_name(
    domain_name: &str,
    variant: &str,
    parameter_name: &str,
) -> String {
    format!("field${}${}${}", domain_name, variant, parameter_name)
}

pub(super) fn enum_variant_destructor_name(domain_name: &str, variant: &str) -> String {
    format!("variant${}${}", domain_name, variant)
}
