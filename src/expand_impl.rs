use std::path::PathBuf;

use im_rc::Vector;
use proc_macro2::{TokenTree, TokenStream};

use crate::{Resolver, Error, attrs};

pub(crate) fn expand_impl<R: Resolver>(
    content: &mut Vec<syn::Item>,
    resolver: &mut R,
    modules_stack: Vector<syn::Ident>,
    dirs: Vector<PathBuf>,
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

        let mut dirs = dirs.clone();

        let len_hint = dirs.len()
            + std::convert::identity::<usize>(dirs.iter().map(|x| x.as_os_str().len()).sum())
            + 8
            + chunk.as_os_str().len();

        let mut module_file_nomod = PathBuf::with_capacity(len_hint);
        let mut module_file_mod = PathBuf::with_capacity(len_hint);

        for x in &dirs {
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

            for x in &dirs {
                module_file_explicit.push(x);
            }
            module_file_explicit.push(explicit_path);

            dirs_candidate = Vector::new();
            if let Some(parent) = module_file_explicit.parent() {
                dirs_candidate.push_back(parent.to_owned());
            }
            dirs_candidate.push_back(PathBuf::from(chunk.clone()));

            if let Some(cfg) = cfg {
                let cfg: syn::Meta = syn::parse2(cfg).map_err(|e| Error::SynParseError {
                    module: mod_syn_path.clone(),
                    e,
                })?;
                if let Some(x) =
                    resolver.resolve(mod_syn_path.clone(), module_file_explicit, Some(cfg))?
                {
                    if inner.is_some() {
                        return Err(Error::MultipleExplicitPathsSpecifiedForOneModule {
                            module: mod_syn_path,
                        });
                    }
                    inner = Some(Some(x));
                }
            } else {
                inner = Some(resolver.resolve(mod_syn_path.clone(), module_file_explicit, None)?);
            }
        }

        let inner = if let Some(i) = inner {
            dirs = dirs_candidate;
            i
        } else {
            dirs.push_back(PathBuf::from(chunk));
            let inner_nomod = resolver.resolve(mod_syn_path.clone(), module_file_nomod, None);
            match inner_nomod {
                Ok(_) => (),
                Err(Error::FailedToOpenFile { .. }) => (),
                Err(e) => return Err(e),
            }
            let inner_mod = resolver.resolve(mod_syn_path.clone(), module_file_mod.clone(), None);
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
                (Ok(Some(x)), _) => Some(x),
                (_, Ok(Some(x))) => Some(x),
                (Err(e), _) => return Err(e),
                (_, Err(e)) => return Err(e),
            }
        };

        if let Some(inner) = inner {
            let mut items = inner.items;

            for attr in inner.attrs {
                attrs.push(attr);
            }

            expand_impl(&mut items, resolver, inner_stack, dirs)?;

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
    }
    Ok(())
}
