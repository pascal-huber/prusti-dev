use crate::encoder::errors::{SpannedEncodingError, SpannedEncodingResult};
use vir_crate::{
    high as vir_high,
    middle::{
        self as vir_mid,
        operations::{
            ToMiddleExpression, ToMiddlePredicate, ToMiddleRvalue, ToMiddleStatementLowerer,
        },
    },
};

impl<'v, 'tcx> ToMiddleStatementLowerer for crate::encoder::Encoder<'v, 'tcx> {
    type Error = SpannedEncodingError;

    fn to_middle_statement_position(
        &self,
        position: vir_high::Position,
    ) -> SpannedEncodingResult<vir_mid::Position> {
        assert!(!position.is_default());
        Ok(position)
    }

    fn to_middle_statement_expression(
        &self,
        expression: vir_high::Expression,
    ) -> SpannedEncodingResult<vir_mid::Expression> {
        expression.to_middle_expression(self)
    }

    fn to_middle_statement_predicate(
        &self,
        predicate: vir_high::Predicate,
    ) -> SpannedEncodingResult<vir_mid::Predicate> {
        predicate.to_middle_predicate(self)
    }

    fn to_middle_statement_rvalue(
        &self,
        rvalue: vir_high::Rvalue,
    ) -> SpannedEncodingResult<vir_mid::Rvalue> {
        rvalue.to_middle_rvalue(self)
    }

    fn to_middle_statement_statement_leak_all(
        &self,
        _statement: vir_high::LeakAll,
    ) -> SpannedEncodingResult<vir_mid::Statement> {
        unreachable!("leak-all statement cannot be lowered")
    }
}
