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

    fn to_middle_statement_operand(
        &self,
        operand: vir_high::Operand,
    ) -> Result<vir_mid::Operand, Self::Error> {
        operand.to_middle_rvalue(self)
    }

    // fn to_middle_statement_variable_decl(
    //     &self,
    //     _statement: vir_high::VariableDecl,
    // ) -> SpannedEncodingResult<vir_mid::Statement> {
    //     unimplemented!("VariableDecl to_middle_statement");
    // }

    fn to_middle_statement_variable_decl(
        &self,
        _: vir_high::VariableDecl,
    ) -> Result<vir_mid::VariableDecl, <Self as ToMiddleStatementLowerer>::Error> {
        todo!()
    }

    // fn to_middle_statement_ghost_assignment(
    //     &self,
    //     _statement: vir_high::GhostAssignment,
    // ) -> SpannedEncodingResult<vir_mid::GhostAssignment> {
    //     unimplemented!()
    //     // unreachable!("leak-all statement cannot be lowered")
    // }

    //     fn to_middle_statement_statement_end_lft(
    //         &self,
    //         endlft: vir_high::EndLft,
    //     ) -> SpannedEncodingResult<vir_mid::Statement> {
    // //        endlft.to_middle_statement_end_lft(self)
    //         unreachable!("end lft not yet lowerable to mid")
    //     }
    //
    //     fn to_middle_statement_statement_new_lft(
    //         &self,
    //
    //     ) -> SpannedEncodingResult<vir_mid::Statement> {
    //         unreachable!("end lft not yet lowerable to mid")
    //     }
    //
}
