use crate::specifications::common::{
    SpecificationId,
    SpecificationIdGenerator,
};
use crate::specifications::untyped;
use proc_macro2::{TokenStream, Span};
use syn::{Type, punctuated::Punctuated, Pat, Token};
use syn::spanned::Spanned;
use quote::{quote_spanned, format_ident};
use crate::specifications::preparser::{
    parse_prusti,
    parse_prusti_pledge, parse_prusti_assert_pledge,
};

pub(crate) struct AstRewriter {
    spec_id_generator: SpecificationIdGenerator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecItemType {
    Precondition,
    Postcondition,
    Pledge,
    Predicate,
}

impl std::fmt::Display for SpecItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecItemType::Precondition => write!(f, "pre"),
            SpecItemType::Postcondition => write!(f, "post"),
            SpecItemType::Pledge => write!(f, "pledge"),
            SpecItemType::Predicate => write!(f, "pred"),
        }
    }
}

impl AstRewriter {
    pub(crate) fn new() -> Self {
        Self {
            spec_id_generator: SpecificationIdGenerator::new(),
        }
    }

    pub fn generate_spec_id(&mut self) -> SpecificationId {
        self.spec_id_generator.generate()
    }

    /// Check whether function `item` contains a parameter called `keyword`. If
    /// yes, return its span.
    fn check_contains_keyword_in_params(&self, item: &untyped::AnyFnItem, keyword: &str) -> Option<Span> {
        for param in &item.sig().inputs {
            if let syn::FnArg::Typed(syn::PatType {
                    pat,
                    ..
                }) = param {
                if let syn::Pat::Ident(syn::PatIdent { ident, .. }) = &**pat {
                    if ident == keyword {
                        return Some(param.span());
                    }
                }
            }
        }
        None
    }

    fn generate_result_arg(&self, item: &untyped::AnyFnItem) -> syn::FnArg {
        let item_span = item.span();
        let output_ty = match &item.sig().output {
            syn::ReturnType::Default => parse_quote_spanned!(item_span=> ()),
            syn::ReturnType::Type(_, ty) => ty.clone(),
        };
        let fn_arg = syn::FnArg::Typed(
            syn::PatType {
                attrs: Vec::new(),
                pat: Box::new(parse_quote_spanned!(item_span=> result)),
                colon_token: syn::Token![:](item.sig().output.span()),
                ty: output_ty,
            }
        );
        fn_arg
    }

    /// Turn an expression into the appropriate function
    pub fn generate_spec_item_fn(
        &mut self,
        spec_type: SpecItemType,
        spec_id: SpecificationId,
        expr: TokenStream,
        item: &untyped::AnyFnItem,
    ) -> syn::Result<syn::Item> {
        if let Some(span) = self.check_contains_keyword_in_params(item, "result") {
            return Err(syn::Error::new(
                span,
                "it is not allowed to use the keyword `result` as a function argument".to_string(),
            ));
        }
        let item_span = expr.span();
        let item_name = syn::Ident::new(
            &format!("prusti_{}_item_{}_{}", spec_type, item.sig().ident, spec_id),
            item_span,
        );
        let spec_id_str = spec_id.to_string();

        // about the span and expression chosen here:
        // - `item_span` is set to `expr.span()` so that any errors reported
        //   for the spec item will be reported on the span of the expression
        //   written by the user
        // - `((#expr) : bool)` syntax is used to report type errors in the
        //   expression with the correct error message, i.e. that the expected
        //   type is `bool`, not that the expected *return* type is `bool`
        // - `!!(...)` is used to fix an edge-case when the expression consists
        //   of a single identifier; without the double negation, the `Return`
        //   terminator in MIR has a span set to the one character just after
        //   the identifier
        let mut spec_item: syn::ItemFn = parse_quote_spanned! {item_span=>
            #[allow(unused_must_use, unused_parens, unused_variables, dead_code)]
            #[prusti::spec_only]
            #[prusti::spec_id = #spec_id_str]
            fn #item_name() -> bool {
                !!((#expr) : bool)
            }
        };

        spec_item.sig.generics = item.sig().generics.clone();
        spec_item.sig.inputs = item.sig().inputs.clone();
        match spec_type {
            SpecItemType::Postcondition | SpecItemType::Pledge => {
                let fn_arg = self.generate_result_arg(item);
                spec_item.sig.inputs.push(fn_arg);
            },
            _ => (),
        }
        Ok(syn::Item::Fn(spec_item))
    }

    /// Parse an assertion into a Rust expression
    pub fn process_assertion(
        &mut self,
        spec_type: SpecItemType,
        spec_id: SpecificationId,
        tokens: TokenStream,
        item: &untyped::AnyFnItem,
    ) -> syn::Result<syn::Item> {
        self.generate_spec_item_fn(
            spec_type,
            spec_id,
            parse_prusti(tokens)?,
            item,
        )
    }

    /// Parse a pledge with lhs into a Rust expression
    pub fn process_pledge(
        &mut self,
        spec_id: SpecificationId,
        tokens: TokenStream,
        item: &untyped::AnyFnItem,
    ) -> syn::Result<syn::Item> {
        self.generate_spec_item_fn(
            SpecItemType::Pledge,
            spec_id,
            parse_prusti_pledge(tokens)?,
            item,
        )
    }

    /// Parse a pledge with lhs into a Rust expression
    pub fn process_assert_pledge(
        &mut self,
        spec_id_lhs: SpecificationId,
        spec_id_rhs: SpecificationId,
        tokens: TokenStream,
        item: &untyped::AnyFnItem,
    ) -> syn::Result<(syn::Item, syn::Item)> {
        let (lhs, rhs) = parse_prusti_assert_pledge(tokens)?;
        let lhs_item = self.generate_spec_item_fn(
            SpecItemType::Pledge,
            spec_id_lhs,
            lhs,
            item,
        )?;
        let rhs_item = self.generate_spec_item_fn(
            SpecItemType::Pledge,
            spec_id_rhs,
            rhs,
            item,
        )?;
        Ok((lhs_item, rhs_item))
    }

    /// Parse a loop invariant into a Rust expression
    pub fn process_loop_invariant(
        &mut self,
        spec_id: SpecificationId,
        tokens: TokenStream,
    ) -> syn::Result<TokenStream> {
        let expr = parse_prusti(tokens)?;
        let spec_id_str = spec_id.to_string();
        Ok(quote_spanned! {expr.span()=>
            {
                #[prusti::spec_only]
                #[prusti::loop_body_invariant_spec]
                #[prusti::spec_id = #spec_id_str]
                || -> bool {
                    #expr
                };
            }
        })
    }

    /// Parse a closure with specifications into a Rust expression
    /// TODO: arguments, result (types are typically not known yet after parsing...)
    pub fn process_closure(
        &mut self,
        inputs: Punctuated<Pat, Token![,]>,
        output: Type,
        preconds: Vec<(SpecificationId, syn::Expr)>,
        postconds: Vec<(SpecificationId, syn::Expr)>,
    ) -> syn::Result<(TokenStream, TokenStream)> {
        let process_cond = |is_post: bool, id: &SpecificationId,
                            assertion: &syn::Expr| -> TokenStream
        {
            let spec_id_str = id.to_string();
            let name = format_ident!("prusti_{}_closure_{}", if is_post { "post" } else { "pre" }, spec_id_str);
            let callsite_span = Span::call_site();
            let result = if is_post && !inputs.empty_or_trailing() {
                quote_spanned! {callsite_span=> , result: #output }
            } else if is_post {
                quote_spanned! {callsite_span=> result: #output }
            } else {
                TokenStream::new()
            };
            quote_spanned! {callsite_span=>
                #[prusti::spec_only]
                #[prusti::spec_id = #spec_id_str]
                fn #name(#inputs #result) {
                    #assertion
                }
            }
        };

        let mut pre_ts = TokenStream::new();
        for (id, precond) in preconds {
            pre_ts.extend(process_cond(false, &id, &precond));
        }

        let mut post_ts = TokenStream::new();
        for (id, postcond) in postconds {
            post_ts.extend(process_cond(true, &id, &postcond));
        }

        Ok((pre_ts, post_ts))
    }

    /// Parse an assertion into a Rust expression
    pub fn process_closure_assertion(
        &mut self,
        spec_id: SpecificationId,
        tokens: TokenStream,
    ) -> syn::Result<syn::Expr> {
        let expr = parse_prusti(tokens)?;
        let spec_id_str = spec_id.to_string();
        let callsite_span = Span::call_site();
        Ok(parse_quote_spanned! {callsite_span=>
            #[allow(unused_must_use, unused_variables)]
            {
                #[prusti::spec_only]
                #[prusti::spec_id = #spec_id_str]
                || -> bool {
                    #expr
                };
            }
        })
    }
}
