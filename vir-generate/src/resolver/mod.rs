use crate::ast::{Include, RawBlock};
use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{fold::Fold, parse_quote};

pub(crate) fn expand(
    ir: syn::ItemMod,
    components: &syn::ItemMod,
) -> (syn::ItemMod, Vec<syn::Error>) {
    let mut expander = Expander {
        components,
        errors: Vec::new(),
    };
    let expanded_ir = expander.fold_item_mod(ir);
    (expanded_ir, expander.errors)
}

struct Expander<'a> {
    components: &'a syn::ItemMod,
    errors: Vec<syn::Error>,
}

impl<'a> Expander<'a> {
    fn expand_include(&mut self, items: &mut Vec<syn::Item>, include: Include) -> syn::Result<()> {
        let module = self.find_component(&include.path)?;
        for imported_type in include.imported_types {
            let mut found = false;
            for item in module {
                match item {
                    syn::Item::Struct(syn::ItemStruct { ident, .. })
                    | syn::Item::Enum(syn::ItemEnum { ident, .. })
                    | syn::Item::Trait(syn::ItemTrait { ident, .. })
                        if ident == &imported_type.name =>
                    {
                        let mut item = item.clone();
                        match &mut item {
                            syn::Item::Struct(syn::ItemStruct { ident, attrs, .. })
                            | syn::Item::Enum(syn::ItemEnum { ident, attrs, .. }) => {
                                *ident = imported_type.alias.clone();
                                if !include.derive_macros.is_empty() {
                                    let mut derive_idents = TokenStream::new();
                                    for derive_macro in &include.derive_macros {
                                        derive_idents.extend(quote! {#derive_macro,});
                                    }
                                    attrs.push(parse_quote!(#[derive(#derive_idents)]))
                                }
                            }
                            syn::Item::Trait(syn::ItemTrait { ident, .. }) => {
                                *ident = imported_type.alias.clone();
                            }
                            _ => unreachable!(),
                        }
                        items.push(item);
                        found = true;
                    }
                    syn::Item::Impl(syn::ItemImpl {
                        trait_: Some((_, path, _)),
                        ..
                    }) if path.is_ident(&imported_type.name) => {
                        let mut item = item.clone();
                        match &mut item {
                            syn::Item::Impl(syn::ItemImpl {
                                trait_: Some((_, path, _)),
                                ..
                            }) => {
                                *path = imported_type.alias.clone().into();
                            }
                            _ => unreachable!(),
                        }
                        items.push(item);
                    }
                    syn::Item::Impl(syn::ItemImpl { self_ty, .. }) => match &**self_ty {
                        syn::Type::Path(syn::TypePath { qself: None, path })
                            if path.is_ident(&imported_type.name) =>
                        {
                            let mut item = item.clone();
                            match &mut item {
                                syn::Item::Impl(syn::ItemImpl { self_ty, .. }) => {
                                    *self_ty = Box::new(syn::Type::Path(syn::TypePath {
                                        qself: None,
                                        path: imported_type.alias.clone().into(),
                                    }));
                                }
                                _ => unreachable!(),
                            }
                            items.push(item);
                        }
                        _ => {}
                    },
                    syn::Item::Macro(syn::ItemMacro { mac, .. })
                        if mac.path.is_ident("vir_raw_block") =>
                    {
                        let block = syn::parse2::<RawBlock>(mac.tokens.clone())?;
                        if block.name == imported_type.name {
                            for item in block.content {
                                items.push(item.clone());
                            }
                            found = true;
                        }
                    }
                    _ => {}
                }
            }
            if !found {
                return Err(syn::Error::new(
                    imported_type.name.span(),
                    format!("not found {}", imported_type.name),
                ));
            }
        }
        Ok(())
    }
    fn find_component(&self, path: &syn::Path) -> syn::Result<&[syn::Item]> {
        let mut current_mod = self.components;
        for segment in &path.segments {
            let (_, content) = current_mod
                .content
                .as_ref()
                .expect("bug: expander did not expand all modules");
            let mut found = false;
            for item in content {
                if let syn::Item::Mod(module) = item {
                    if module.ident == segment.ident {
                        current_mod = module;
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return Err(syn::Error::new(segment.span(), "not found matching module"));
            }
        }
        let (_, content) = current_mod
            .content
            .as_ref()
            .expect("bug (2): expander did not expand all modules");
        Ok(content)
    }
}

impl<'a> Fold for Expander<'a> {
    fn fold_item_mod(&mut self, mut item_mod: syn::ItemMod) -> syn::ItemMod {
        if let Some((brace, content)) = item_mod.content {
            let mut new_content = Vec::new();
            for item in content {
                match item {
                    syn::Item::Macro(macro_item) if macro_item.mac.path.is_ident("vir_include") => {
                        match syn::parse2::<Include>(macro_item.mac.tokens) {
                            Ok(include) => {
                                if let Err(error) = self.expand_include(&mut new_content, include) {
                                    self.errors.push(error);
                                }
                            }
                            Err(error) => {
                                self.errors.push(error);
                            }
                        }
                    }
                    _ => {
                        new_content.push(syn::fold::fold_item(self, item));
                    }
                }
            }
            item_mod.content = Some((brace, new_content));
        }
        item_mod
    }
}
