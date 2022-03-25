use std::path::PathBuf;

use im_rc::Vector;
use proc_macro2::{TokenTree, TokenStream};

use crate::{Resolver, Error, attrs};

pub(crate) fn expand_impl<R: Resolver>(
    content: &mut Vec<syn::Item>,
    resolver: &mut R,
    modules_stack: Vector<syn::Ident>,
    relative_path_where_to_look_for_nested_modules_naturally: Vector<PathBuf>,
    relative_path_where_to_look_for_nested_modules_when_using_path_attribute: Vector<PathBuf>,
) -> Result<(), Error> {
    for item in content {
        let (item_mod, semicolon_after_module_declaration) = match item {
            syn::Item::Mod(ref mut item_mod) => {
                match (&item_mod.content, item_mod.semi) {
                    (None, None) => panic!("A module without both `{{}}` and `;`?"),
                    (Some(_), Some(_)) => panic!("A module with both `{{}}` and `;`?"),
                    (Some(_), None) => continue, // inline module
                    (None, Some(semi)) => (item_mod, semi),
                }
            }
            _ => continue,
        };

        let id = item_mod.ident.clone();

        let mut inner_stack = modules_stack.clone();
        inner_stack.push_back(id.clone());

        let chunk = PathBuf::from(format!("{}", id));
        let chunk_rs = PathBuf::from(format!("{}.rs", id));

        let mut dirs_nat = relative_path_where_to_look_for_nested_modules_naturally.clone();
        let mut dirs_attr = relative_path_where_to_look_for_nested_modules_when_using_path_attribute.clone();

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

        let mut attrs = Vec::with_capacity(item_mod.attrs.len());

        // outer level of Option: whether we have expansion result based on path attribute results
        // inner level of Option: whether expansion resulted in actual code
        let mut inner: Option<Option<syn::File>> = None;

        let mut dirs_candidate = Vector::new();

        let mut path_attrs: Vec<(Vec<TokenTree>, Option<TokenStream>)> = Vec::new();
        attrs::extract_path_and_cfg_path_attrs(
            &item_mod.attrs,
            &mut path_attrs,
            &mod_syn_path,
            &mut attrs,
        )?;

        for (tt, cfg) in path_attrs {
            if cfg.is_none() && inner.is_some() {
                return Err(Error::MultipleExplicitPathsSpecifiedForOneModule {
                    module: mod_syn_path,
                });
            }

            let explicit_path = attrs::extract_path_from_attr(tt, &mod_syn_path)?;

            let mut module_file_explicit = PathBuf::with_capacity(len_hint);

            for x in &dirs_attr {
                module_file_explicit.push(x);
            }
            module_file_explicit.push(explicit_path);

            dirs_candidate = Vector::new();
            if let Some(parent) = module_file_explicit.parent() {
                dirs_candidate.push_back(parent.to_owned());
            }

            if let Some(cfg) = cfg {
                let cfg: syn::Meta = syn::parse2(cfg).map_err(|e| Error::SynParseError {
                    module: mod_syn_path.clone(),
                    e,
                })?;
                if resolver.check_cfg(cfg).map_err(|e|Error::ErrorFromCallback{e})? {
                    if inner.is_some() {
                        return Err(Error::MultipleExplicitPathsSpecifiedForOneModule {
                            module: mod_syn_path,
                        });
                    }
                    inner = Some(resolver.resolve(mod_syn_path.clone(), module_file_explicit)?);
                }
            } else {
                inner = Some(resolver.resolve(mod_syn_path.clone(), module_file_explicit)?);
            }
        }

        let inner = if let Some(i) = inner {
            dirs_attr = dirs_candidate.clone();
            dirs_nat = dirs_candidate;
            i
        } else {
            let inner_nomod = resolver.resolve(mod_syn_path.clone(), module_file_nomod);
            match inner_nomod {
                Ok(_) => (),
                Err(Error::FailedToOpenFile { .. }) => (),
                Err(e) => return Err(e),
            }
            let inner_mod = resolver.resolve(mod_syn_path.clone(), module_file_mod.clone());
            match inner_mod {
                Ok(_) => (),
                Err(Error::FailedToOpenFile { .. }) => (),
                Err(e) => return Err(e),
            }
            match (inner_nomod, inner_mod) {
                (Ok(Some(_)), Ok(Some(_))) => {
                    return Err(Error::BothModRsAndNameRsPresent {
                        module: mod_syn_path,
                    })
                }
                (Ok(None), Ok(None)) => None,
                (Ok(Some(x)), _) => {
                    dirs_attr = dirs_nat.clone();
                    dirs_nat.push_back(PathBuf::from(chunk.clone()));
                    Some(x)
                },
                (_, Ok(Some(x))) => {
                    dirs_nat.push_back(PathBuf::from(chunk.clone()));
                    dirs_attr = dirs_nat.clone();
                    Some(x)
                },
                (Err(e), _) => return Err(e),
                (_, Err(e)) => return Err(e),
            }
        };

        if let Some(inner) = inner {
            let mut items = inner.items;

            for attr in inner.attrs {
                attrs.push(attr);
            }

            //dbg!(&inner_stack, &dirs_nat, &dirs_attr);
            expand_impl(&mut items, resolver, inner_stack, dirs_nat, dirs_attr)?;

            let new_mod = syn::ItemMod {
                attrs,
                vis: item_mod.vis.clone(),
                mod_token: item_mod.mod_token,
                ident: id,
                content: Some((
                    syn::token::Brace(semicolon_after_module_declaration.span),
                    items,
                )),
                semi: None,
            };
            *item = syn::Item::Mod(new_mod);
        }
    } // end loop
    Ok(())
}
