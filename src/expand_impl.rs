use std::path::PathBuf;

use im_rc::Vector;
use proc_macro2::{TokenStream, TokenTree};
use syn::{punctuated::Punctuated, spanned::Spanned, MetaList};

use crate::{attrs, Error, ErrorCase, Resolver};

pub(crate) fn expand_impl<R: Resolver>(
    content: &mut Vec<syn::Item>,
    resolver: &mut R,
    modules_stack: Vector<syn::Ident>,
    relative_path_where_to_look_for_nested_modules_naturally: Vector<PathBuf>,
    relative_path_where_to_look_for_nested_modules_when_using_path_attribute: Vector<PathBuf>,
    multimodule_mode: bool,
) -> Result<(), Error> {
    let mut multimodule_tmp_container: Vec<syn::Item> = if multimodule_mode {
        Vec::with_capacity(content.len())
    } else {
        Vec::new()
    };
    'items_loop: for item in &mut *content {
        let (item_mod, semicolon_after_module_declaration) = match item {
            syn::Item::Mod(ref mut item_mod) => {
                match (&item_mod.content, item_mod.semi) {
                    (None, None) => panic!("A module without both `{{}}` and `;`?"),
                    (Some(_), Some(_)) => panic!("A module with both `{{}}` and `;`?"),
                    (Some(_), None) => continue, // inline module
                    (None, Some(semi)) => (item_mod, semi),
                }
            }
            _ => {
                if multimodule_mode {
                    multimodule_tmp_container.push(item.clone());
                }
                continue;
            }
        };

        let id = item_mod.ident.clone();

        let mut inner_stack = modules_stack.clone();
        inner_stack.push_back(id.clone());

        let chunk = PathBuf::from(format!("{}", id));
        let chunk_rs = PathBuf::from(format!("{}.rs", id));

        let mut dirs_nat = relative_path_where_to_look_for_nested_modules_naturally.clone();
        let mut dirs_attr =
            relative_path_where_to_look_for_nested_modules_when_using_path_attribute.clone();

        let len_hint = dirs_nat.len()
            + std::convert::identity::<usize>(dirs_attr.iter().map(|x| x.as_os_str().len()).sum())
            + 8
            + chunk.as_os_str().len();

        let mut module_file_nomod = PathBuf::with_capacity(len_hint);
        let mut module_file_mod = PathBuf::with_capacity(len_hint);

        for x in &dirs_nat {
            module_file_mod.push(x);
            module_file_nomod.push(x);
        }
        module_file_mod.push(&chunk);
        module_file_mod.push("mod.rs");
        module_file_nomod.push(&chunk_rs);

        let mod_syn_path = syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::from_iter(inner_stack.iter().map(|x| {
                syn::PathSegment {
                    ident: x.clone(),
                    arguments: syn::PathArguments::None,
                }
            })),
        };

        let err = |c| Error {
            module: mod_syn_path.clone(),
            inner: c,
        };

        let mut attrs = Vec::with_capacity(item_mod.attrs.len());

        struct ExpandedModuleInfo {
            result: Option<syn::File>,
            /// Root path to use for recursive expansions for inner `#[path]` modules ("attribute" path)
            dirs_attr: Vector<PathBuf>,
            /// Root path to use for recursive expansions for inner modules without `#[path]` ("natural" path)
            dirs_nat: Vector<PathBuf>,
            cfg: Option<syn::Meta>,
        }

        // outer level of Vec: expansion results based on multiple path attributes or `name/mod.rs` vs `name.rs` distinction.
        // inner level of Option: whether the expansion resulted in actual code or failure to open a file
        let mut expansion_candidates: Vec<ExpandedModuleInfo> = Vec::with_capacity(1);

        let mut path_attrs: Vec<(Vec<TokenTree>, Option<TokenStream>)> = Vec::new();
        let mut cfg_attrs: Vec<TokenStream> = Vec::new();
        attrs::read_and_process_attributes(
            &item_mod.attrs,
            &mut path_attrs,
            &mut attrs,
            &mut cfg_attrs,
        )
        .map_err(|e| Error {
            module: mod_syn_path.clone(),
            inner: ErrorCase::AttrParseError(e),
        })?;

        for cfg in cfg_attrs {
            let cfg: syn::Meta = syn::parse2(cfg).map_err(|e| err(ErrorCase::SynParseError(e)))?;
            if !resolver.check_cfg(cfg).map_err(|e| Error {
                module: mod_syn_path.clone(),
                inner: ErrorCase::ErrorFromCallback(e),
            })? {
                continue 'items_loop;
            }
        }

        let mut need_to_try_natural_file_locations = true;
        let mut accumulated_cfgs = Vec::<syn::Meta>::new();

        for (tt, cfg) in path_attrs {
            if cfg.is_none() && !expansion_candidates.is_empty() && !multimodule_mode {
                return Err(err(ErrorCase::MultipleExplicitPathsSpecifiedForOneModule));
            }

            let explicit_path = attrs::extract_path_from_attr(tt, &mod_syn_path)?;

            let mut module_file_explicit = PathBuf::with_capacity(len_hint);

            for x in &dirs_attr {
                module_file_explicit.push(x);
            }
            module_file_explicit.push(explicit_path);

            let mut dirs_candidate = Vector::new();
            if let Some(parent) = module_file_explicit.parent() {
                dirs_candidate.push_back(parent.to_owned());
            }

            if let Some(cfg) = cfg {
                let cfg: syn::Meta =
                    syn::parse2(cfg).map_err(|e| err(ErrorCase::SynParseError(e)))?;
                if resolver
                    .check_cfg(cfg.clone())
                    .map_err(|e| err(ErrorCase::ErrorFromCallback(e)))?
                {
                    if !expansion_candidates.is_empty() && !multimodule_mode {
                        return Err(err(ErrorCase::MultipleExplicitPathsSpecifiedForOneModule));
                    }
                    if !multimodule_mode {
                        need_to_try_natural_file_locations = false;
                    }
                    let result = resolver.resolve(mod_syn_path.clone(), module_file_explicit)?;
                    expansion_candidates.push(ExpandedModuleInfo {
                        result,
                        dirs_attr: dirs_candidate.clone(),
                        dirs_nat: dirs_candidate,
                        cfg: Some(cfg.clone()),
                    });
                    accumulated_cfgs.push(cfg);
                }
            } else {
                need_to_try_natural_file_locations = false;
                let result = resolver.resolve(mod_syn_path.clone(), module_file_explicit)?;
                expansion_candidates.push(ExpandedModuleInfo {
                    result,
                    dirs_attr: dirs_candidate.clone(),
                    dirs_nat: dirs_candidate,
                    cfg: None,
                });
            }
        }

        assert!(multimodule_mode || expansion_candidates.len() <= 1);

        if need_to_try_natural_file_locations {
            let inner_nomod = resolver.resolve(mod_syn_path.clone(), module_file_nomod);
            match inner_nomod {
                Ok(_) => (),
                Err(Error {
                    inner: ErrorCase::FailedToOpenFile { .. },
                    ..
                }) => (),
                Err(e) => return Err(e),
            }
            let inner_mod = resolver.resolve(mod_syn_path.clone(), module_file_mod.clone());
            match inner_mod {
                Ok(_) => (),
                Err(Error {
                    inner: ErrorCase::FailedToOpenFile { .. },
                    ..
                }) => (),
                Err(e) => return Err(e),
            }
            let result = match (inner_nomod, inner_mod) {
                (Ok(Some(_)), Ok(Some(_))) => {
                    return Err(err(ErrorCase::BothModRsAndNameRsPresent))
                }
                (Ok(None), Ok(None)) => None,
                (Ok(Some(x)), _) => {
                    dirs_attr = dirs_nat.clone();
                    dirs_nat.push_back(PathBuf::from(chunk.clone()));
                    Some(x)
                }
                (_, Ok(Some(x))) => {
                    dirs_nat.push_back(PathBuf::from(chunk.clone()));
                    dirs_attr = dirs_nat.clone();
                    Some(x)
                }
                (Err(ref e1), Err(ref e2))
                    if multimodule_mode
                        && matches!(
                            (&e1.inner, &e2.inner),
                            (
                                ErrorCase::FailedToOpenFile { .. },
                                ErrorCase::FailedToOpenFile { .. }
                            )
                        ) =>
                {
                    None
                }
                (Err(e), _) => return Err(e),
                (_, Err(e)) => return Err(e),
            };
            let some_span = item_mod.span();
            let cfg = if multimodule_mode && !accumulated_cfgs.is_empty() {
                Some(syn::Meta::List(MetaList {
                    path: simple_path(some_span, "not"),
                    paren_token: syn::token::Paren { span: some_span },
                    nested: Punctuated::from_iter([syn::NestedMeta::Meta(syn::Meta::List(
                        MetaList {
                            path: simple_path(some_span, "any"),
                            paren_token: syn::token::Paren { span: some_span },
                            nested: Punctuated::from_iter(
                                accumulated_cfgs
                                    .iter()
                                    .map(|x| syn::NestedMeta::Meta(x.clone())),
                            ),
                        },
                    ))]),
                }))
            } else {
                None
            };
            expansion_candidates.push(ExpandedModuleInfo {
                result,
                dirs_attr,
                dirs_nat,
                cfg,
            });
        }

        assert!(!expansion_candidates.is_empty());
        if !multimodule_mode {
            assert_eq!(expansion_candidates.len(), 1);
        }

        for ExpandedModuleInfo {
            result,
            dirs_attr,
            dirs_nat,
            cfg,
        } in expansion_candidates.into_iter()
        {
            if let Some(inner) = result {
                let mut inner_items = inner.items;

                let mut attrs_copy = attrs.clone();

                let some_span = item_mod.span();
                if let Some(cfg) = cfg {
                    if multimodule_mode {
                        attrs_copy.push(syn::Attribute {
                            // unsure what to do with the spans of the newly generated tokens
                            // just plugging whatever matches the signature and what I have found the first.
                            pound_token: syn::token::Pound { spans: [some_span] },
                            style: syn::AttrStyle::Outer,
                            bracket_token: syn::token::Bracket { span: some_span },
                            path: simple_path(some_span, "cfg"),
                            tokens: quote::quote!((#cfg)),
                        })
                    }
                }

                for attr in inner.attrs {
                    attrs_copy.push(attr);
                }

                //dbg!(&inner_stack, &dirs_nat, &dirs_attr);
                expand_impl(
                    &mut inner_items,
                    resolver,
                    inner_stack.clone(),
                    dirs_nat,
                    dirs_attr,
                    multimodule_mode,
                )?;

                let vis = item_mod.vis.clone();
                let mod_token = item_mod.mod_token.clone();
                let new_mod = syn::ItemMod {
                    attrs: attrs_copy,
                    vis,
                    mod_token,
                    ident: id.clone(),
                    content: Some((
                        syn::token::Brace(semicolon_after_module_declaration.span),
                        inner_items,
                    )),
                    semi: None,
                };

                if !multimodule_mode {
                    *item = syn::Item::Mod(new_mod);
                    continue 'items_loop;
                } else {
                    multimodule_tmp_container.push(syn::Item::Mod(new_mod));
                }
            }
        }
    } // end loop
    if multimodule_mode {
        *content = multimodule_tmp_container;
    }
    Ok(())
}

fn simple_path(span: proc_macro2::Span, name: &'static str) -> syn::Path {
    syn::Path {
        leading_colon: None,
        segments: Punctuated::from_iter([syn::punctuated::Pair::new(
            syn::PathSegment {
                ident: syn::Ident::new(name, span),
                arguments: syn::PathArguments::None,
            },
            None,
        )]),
    }
}
