use crate::encoder::errors::{SpannedEncodingError, SpannedEncodingResult};
use vir_crate::{
    high as vir_high,
    middle::{
        self as vir_mid,
        operations::{ToMiddleExpression, ToMiddleRvalueLowerer},
    },
};

impl<'v, 'tcx> ToMiddleRvalueLowerer for crate::encoder::Encoder<'v, 'tcx> {
    type Error = SpannedEncodingError;

    fn to_middle_rvalue_expression(
        &self,
        expression: vir_high::Expression,
    ) -> SpannedEncodingResult<vir_mid::Expression> {
        expression.to_middle_expression(self)
    }

    fn to_middle_rvalue_binary_op_kind(
        &self,
        kind: vir_high::BinaryOpKind,
    ) -> Result<vir_mid::BinaryOpKind, Self::Error> {
        kind.to_middle_expression(self)
    }

    fn to_middle_rvalue_unary_op_kind(
        &self,
        kind: vir_high::UnaryOpKind,
    ) -> Result<vir_mid::UnaryOpKind, Self::Error> {
        kind.to_middle_expression(self)
    }
}
