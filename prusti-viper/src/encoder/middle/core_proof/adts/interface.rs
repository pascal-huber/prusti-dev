use crate::encoder::{errors::SpannedEncodingResult, middle::core_proof::lowerer::Lowerer};
use vir_crate::low::{self as vir_low};

use super::types::{
    constructor_call, constructor_name, destructor_call,
     AdtConstructorKind, constant_destructor_name, alternative_struct_destructor_name, enum_variant_destructor_call,
};

pub(in super::super) trait AdtsInterface {
    fn adt_constructor_constant_call(
        &mut self,
        domain_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    fn adt_destructor_constant_name(&mut self, domain_name: &str) -> SpannedEncodingResult<String>;
    fn adt_constructor_struct_name(&mut self, domain_name: &str) -> SpannedEncodingResult<String>;
    fn adt_destructor_struct_call(
        &mut self,
        domain_name: &str,
        field_name: &str,
        field_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression>;
    fn adt_constructor_struct_alternative_name(
        &mut self,
        domain_name: &str,
        variant: &str,
    ) -> SpannedEncodingResult<String>;
    fn adt_destructor_struct_alternative_name(
        &mut self,
        domain_name: &str,
        variant: &str,
        field_name: &str,
    ) -> SpannedEncodingResult<String>;
    fn adt_destructor_enum_variant_call(
        &mut self,
        domain_name: &str,
        variant: &str,
        variant_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression>;
}

impl<'p, 'v: 'p, 'tcx: 'v> AdtsInterface for Lowerer<'p, 'v, 'tcx> {
    fn adt_constructor_constant_call(
        &mut self,
        domain_name: &str,
        arguments: Vec<vir_low::Expression>,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        Ok(constructor_call(domain_name, &AdtConstructorKind::Constant, arguments))
    }
    fn adt_destructor_constant_name(&mut self, domain_name: &str) -> SpannedEncodingResult<String> {
        Ok(constant_destructor_name(domain_name))
    }
    fn adt_constructor_struct_name(&mut self, domain_name: &str) -> SpannedEncodingResult<String> {
        Ok(constructor_name(domain_name, &AdtConstructorKind::Struct))
    }
    fn adt_destructor_struct_call(
        &mut self,
        domain_name: &str,
        field_name: &str,
        field_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        Ok(destructor_call(
            domain_name,
            &AdtConstructorKind::Struct,
            field_name,
            field_type,
            argument,
        ))
    }
    fn adt_constructor_struct_alternative_name(
        &mut self,
        domain_name: &str,
        variant: &str,
    ) -> SpannedEncodingResult<String> {
        Ok(constructor_name(domain_name, &AdtConstructorKind::AlternativeStruct { name: variant.to_string() }))
    }
    fn adt_destructor_struct_alternative_name(
        &mut self,
        domain_name: &str,
        variant: &str,
        field_name: &str,
    ) -> SpannedEncodingResult<String> {
        Ok(alternative_struct_destructor_name(domain_name, variant, field_name))
    }
    fn adt_destructor_enum_variant_call(
        &mut self,
        domain_name: &str,
        variant: &str,
        variant_type: vir_low::Type,
        argument: vir_low::Expression,
    ) -> SpannedEncodingResult<vir_low::Expression> {
        Ok(enum_variant_destructor_call(
            domain_name,
            variant,
            variant_type,
            argument,
        ))
    }
}
