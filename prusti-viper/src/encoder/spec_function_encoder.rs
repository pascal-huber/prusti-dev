use crate::encoder::snapshot::interface::SnapshotEncoderInterface;
use crate::encoder::{Encoder, borrows::ProcedureContract};
use crate::encoder::errors::{ErrorCtxt, SpannedEncodingResult, WithSpan};
use crate::encoder::borrows::compute_procedure_contract;
use crate::encoder::mir_encoder::{MirEncoder, PlaceEncoder};
use crate::encoder::mir::{
    pure::SpecificationEncoderInterface,
    specifications::SpecificationsInterface,
};
use prusti_interface::{
    environment::{
        Procedure
    },
    data::ProcedureDefId,
    specs::typed,
};
use vir_crate::polymorphic as vir;
use vir_crate::polymorphic::ExprIterator;
use rustc_middle::{mir, ty::subst::SubstsRef};
use rustc_span::Span;
use super::encoder::SubstMap;

pub enum SpecFunctionKind {
    Pre,
    Post,
    HistInv
}

pub struct SpecFunctionEncoder<'p, 'v: 'p, 'tcx: 'v> {
    encoder: &'p Encoder<'v, 'tcx>,
    procedure: &'p Procedure<'tcx>,
    span: Span,
    proc_def_id: ProcedureDefId,
    is_closure: bool,
    mir_encoder: MirEncoder<'p, 'v, 'tcx>,
    tymap: &'p SubstMap<'tcx>,
    substs: &'p SubstsRef<'tcx>,
}

impl<'p, 'v: 'p, 'tcx: 'v> SpecFunctionEncoder<'p, 'v, 'tcx> {
    pub fn new(encoder: &'p Encoder<'v, 'tcx>,
               procedure: &'p Procedure<'tcx>,
               tymap: &'p SubstMap<'tcx>,
               substs: &'p SubstsRef<'tcx>) -> Self {
        Self {
            encoder,
            procedure,
            span: procedure.get_span(),
            proc_def_id: procedure.get_id(),
            is_closure: encoder.env().tcx().is_closure(procedure.get_id()),
            mir_encoder: MirEncoder::new(encoder, procedure.get_mir(), procedure.get_id()),
            tymap,
            substs,
        }
    }

    pub fn encode(&self) -> SpannedEncodingResult<Vec<vir::Function>> {
        let _pre_name = self.encoder.encode_spec_func_name(self.procedure.get_id(),
                                                          SpecFunctionKind::Pre);
        let _post_name = self.encoder.encode_spec_func_name(self.procedure.get_id(),
                                                           SpecFunctionKind::Post);

        let specs = if let Some(specs) = self.encoder.get_procedure_specs(self.proc_def_id) {
            specs
        } else {
            return Ok(vec![]);
        };

        let contract = compute_procedure_contract(
            self.proc_def_id,
            self.encoder.env(),
            typed::SpecificationSet::Procedure(specs),
            Some(self.tymap)
        ).with_span(self.span)?.to_def_site_contract();

        let pre_func = self.encode_pre_spec_func(&contract)?;
        let post_func = self.encode_post_spec_func(&contract)?;

        Ok(vec![pre_func, post_func])
    }

    fn encode_pre_spec_func(&self, contract: &ProcedureContract<'tcx>)
        -> SpannedEncodingResult<vir::Function> {
        let mut func_spec: Vec<vir::Expr> = vec![];

        let encoded_args: Vec<vir::LocalVar> = contract
            .args
            .iter()
            .map(|local| self.encode_local((*local).into()))
            .collect::<Result<Vec<_>, _>>()?;

        for item in contract.functional_precondition() {
            let assertion = self.encoder.encode_assertion(
                item,
                None,
                &encoded_args
                    .iter()
                    .map(|e| -> vir::Expr { e.into() }).collect::<Vec<_>>(),
                None,
                true,
                self.proc_def_id,
                self.tymap,
                self.substs,
            )?;
            self.encoder.error_manager().set_error(
                assertion.pos(),
                ErrorCtxt::PureFunctionDefinition,
            );
            func_spec.push(assertion);
        }

        Ok(vir::Function {
            name: self.encoder.encode_spec_func_name(self.procedure.get_id(),
                                                     SpecFunctionKind::Pre),
            type_arguments: Vec::new(), // FIXME: This is probably wrong.
            formal_args: encoded_args.into_iter()
                                     .skip(if self.is_closure { 1 } else { 0 }) // FIXME: "self" is skipped, see TypeEncoder
                                     .collect(),
            return_type: vir::Type::Bool,
            pres: Vec::new(),
            posts: Vec::new(),
            body: Some(self.encoder.patch_snapshots(func_spec.into_iter().conjoin(), self.tymap).with_span(self.span)?),
        })
    }

    fn encode_post_spec_func(&self, contract: &ProcedureContract<'tcx>)
        -> SpannedEncodingResult<vir::Function> {
        let mut func_spec: Vec<vir::Expr> = vec![];

        let encoded_args: Vec<vir::LocalVar> = contract
            .args
            .iter()
            .map(|local| self.encode_local((*local).into()))
            .collect::<Result<Vec<_>, _>>()?;
        let encoded_return = self.encode_local(contract.returned_value.into())?;
        // encoded_args:
        // _1    - closure "self"
        // _2... - additional arguments
        // encoded return: _0

        for item in contract.functional_postcondition() {
            let assertion = self.encoder.encode_assertion(
                item,
                None,
                &encoded_args
                    .iter()
                    .map(|e| -> vir::Expr { e.into() }).collect::<Vec<_>>(),
                Some(&encoded_return.clone().into()),
                true,
                self.proc_def_id,
                self.tymap,
                self.substs,
            )?;
            self.encoder.error_manager().set_error(
                assertion.pos(),
                ErrorCtxt::PureFunctionDefinition,
            );
            func_spec.push(assertion);
        }

        Ok(vir::Function {
            name: self.encoder.encode_spec_func_name(self.procedure.get_id(),
                                                     SpecFunctionKind::Post),
            type_arguments: Vec::new(), // FIXME: This is probably wrong.
            formal_args: encoded_args.into_iter()
                                     .skip(if self.is_closure { 1 } else { 0 }) // FIXME: "self" is skipped, see TypeEncoder
                                     .chain(std::iter::once(encoded_return))
                                     .collect(),
            return_type: vir::Type::Bool,
            pres: Vec::new(),
            posts: Vec::new(),
            body: Some(self.encoder.patch_snapshots(func_spec.into_iter().conjoin(), self.tymap).with_span(self.span)?),
        })
    }

    fn encode_local(&self, local: mir::Local) -> SpannedEncodingResult<vir::LocalVar> {
        let var_name = self.mir_encoder.encode_local_var_name(local);
        let var_type = self
            .encoder
            .encode_snapshot_type(self.mir_encoder.get_local_ty(local), self.tymap)
            .with_span(self.span)?;
        Ok(vir::LocalVar::new(var_name, var_type))
    }
}
