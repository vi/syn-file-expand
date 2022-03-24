use std::path::PathBuf;
use proc_macro2::{TokenTree, TokenStream};
use thiserror::Error;

pub extern crate proc_macro2;
pub extern crate syn;

use im_rc::Vector;

pub type UserError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Error, Debug)]
pub enum PathAttrParseError {
    #[error("#[path] attribute has not exactly two tokens: equal sign and a path")]
    NotExactlyTwoTokens,
    #[error("#[path] attribute's first token is not an equal sign punctuation")]
    FirstTokenIsNotEqualSign,
    #[error("#[path] attribute's second token is not a string literal")]
    SecondTokenIsNotStringLiteral,
    #[error("#[cfg_attr] attribute is not followed by a single round brackets group")]
    CfgAttrNotRoundGroup,
    #[error("#[cfg_attr] attribute does not have exactly two parameters")]
    CfgAttrNotTwoParams,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Cannot open file {path}: {e}")]
    FailedToOpenFile { path: PathBuf, e: std::io::Error },
    #[error("The module have multiple explicit #[path] directives for {module}", module=PathForDisplay(module))]
    MultipleExplicitPathsSpecifiedForOneModule { module: syn::Path },
    #[error("Both name/mod.rs and name.rs present for {module}", module=PathForDisplay(module))]
    BothModRsAndNameRsPresent { module: syn::Path },
    #[error("error parsing path or cfg_attr path attribute in {module}: {e}", module=PathForDisplay(module))]
    PathAttrParseError {
        module: syn::Path,
        e: PathAttrParseError,
    },
    #[error("parsing error: {e} in {module}", module=PathForDisplay(module))]
    SynParseError {
        module: syn::Path,
        e: syn::parse::Error,
    },
    #[error("Error from callback: {e}")]
    ErrorFromCallback { e: UserError },
}

struct PathForDisplay<'a>(&'a syn::Path);
impl<'a> std::fmt::Display for PathForDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        itertools::join(self.0.segments.iter().map(|x| x.ident.to_string()), "::").fmt(f)
    }
}

/// Data and configuration source for the progress of expansion.
///
/// You may specify a closure instead of a manual trait implementation.
pub trait Resolver {
    /// Called by `expand_modules_into_inline_modules` each time a non-inline module is encountered.
    /// `module_name` is full path from the specified file root to the module being expanded,
    /// separated by `::`, including the module name of the file being expanded.
    ///
    /// `relative_path` is pathname of a file the module should be expanded from,
    /// relative to root module you are expanding.
    ///
    /// `relative_path_mod` is alternative location of the file, for `mod.rs` variant.
    ///
    /// `cfg` is x from `#[cfg_attr(x, path=...)]` or None if path is not overridden or unconditionally overridden.
    ///
    /// This callback may be called multiple times if there are multiple `#[path]` or `#[cfg_attr(...,path)]` attributes.
    /// It is error to return more than one Ok(Some) for one module.
    ///
    /// Retuning Ok(None) for all potential files leaves the module unexpanded in `expand_modules_into_inline_modules` output.
    fn resolve(
        &mut self,
        module_name: syn::Path,
        path_relative_to_crate_root: PathBuf,
        cfg: Option<syn::Meta>,
    ) -> Result<Option<syn::File>, Error>;
}

impl<F: FnMut(syn::Path, PathBuf, Option<syn::Meta>) -> Result<Option<syn::File>, Error>> Resolver
    for F
{
    fn resolve(
        &mut self,
        module_name: syn::Path,
        path_relative_to_crate_root: PathBuf,
        cfg: Option<syn::Meta>,
    ) -> Result<Option<syn::File>, Error> {
        (self)(module_name, path_relative_to_crate_root, cfg)
    }
}

/// Take a `syn` representation of a Rust source file and turn into similar `syn` representation,
/// but with `mod something;` expanded into `mod something { ... }`.
///
/// It is low-level IO-agnostic function: your callback is responsible for reading and parsing the data.
pub fn expand_modules_into_inline_modules<R: Resolver>(
    content: &mut syn::File,
    resolver: &mut R,
) -> Result<(), Error> {
    expand_impl(&mut content.items, resolver, Vector::new(), Vector::new())?;
    Ok(())
}

fn expand_impl<R: Resolver>(
    content: &mut Vec<syn::Item>,
    resolver: &mut R,
    modules_stack: Vector<syn::Ident>,
    dirs: Vector<PathBuf>,
) -> Result<(), Error> {
    for item in content {
        let mut replacement_item = None;
        match item {
            syn::Item::Mod(m) => {
                match (&m.content, &m.semi) {
                    (None, None) => panic!("A module without both `{{}}` and `;`?"),
                    (Some(_), Some(_)) => panic!("A module with both `{{}}` and `;`?"),
                    (Some(_), None) => (), // inline module
                    (None, Some(semi)) => {
                        let id = m.ident.clone();

                        let mut inner_stack = modules_stack.clone();
                        inner_stack.push_back(id.clone());

                        let chunk = PathBuf::from(format!("{}", id));
                        let chunk_rs = PathBuf::from(format!("{}.rs", id));

                        let mut dirs = dirs.clone();

                        let len_hint = dirs.len()
                            + std::convert::identity::<usize>(
                                dirs.iter().map(|x| x.as_os_str().len()).sum(),
                            )
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
                            segments: syn::punctuated::Punctuated::from_iter(
                                inner_stack.iter().map(|x| syn::PathSegment {
                                    ident: x.clone(),
                                    arguments: syn::PathArguments::None,
                                }),
                            ),
                        };

                        let mut attrs = Vec::with_capacity(m.attrs.len());

                        let mut inner : Option<Option<syn::File>> = None;

                        let mut dirs_candidate = Vector::new();

                        let mut path_attrs : Vec<(Vec<TokenTree>, Option<TokenStream>)> = Vec::new();
                        for attr in &m.attrs {
                            match &attr.path {
                                x if x.get_ident().map(|x| x.to_string())
                                    == Some("path".to_owned()) =>
                                {
                                    let tt = Vec::<TokenTree>::from_iter(
                                        attr.tokens.clone(),
                                    );
                                    path_attrs.push((tt, None));
                                }
                                x if x.get_ident().map(|x| x.to_string())
                                    == Some("cfg_attr".to_owned()) =>
                                {
                                    let tt = Vec::<TokenTree>::from_iter(
                                        attr.tokens.clone(),
                                    );
                                    if tt.len() != 1 {
                                        return Err(Error::PathAttrParseError {
                                            module: mod_syn_path,
                                            e: PathAttrParseError::CfgAttrNotRoundGroup,
                                        });
                                    }
                                    let t = tt.into_iter().next().unwrap();
                                    let g = match t {
                                        TokenTree::Group(g)
                                            if g.delimiter()
                                                == proc_macro2::Delimiter::Parenthesis =>
                                        {
                                            g
                                        }
                                        _ => {
                                            return Err(Error::PathAttrParseError {
                                                module: mod_syn_path,
                                                e: PathAttrParseError::CfgAttrNotRoundGroup,
                                            })
                                        }
                                    };
                                    let inner =
                                        Vec::<TokenTree>::from_iter(g.stream());
                                    if inner.len() < 3 {
                                        return Err(Error::PathAttrParseError {
                                            module: mod_syn_path,
                                            e: PathAttrParseError::CfgAttrNotTwoParams,
                                        });
                                    }
                                    let mut ts_before_comma = TokenStream::new();
                                    let mut ts_after_comma = Vec::<TokenTree>::new();
                                    let mut comma_encountered = false;
                                    for t in inner.into_iter() {
                                        match t {
                                            TokenTree::Punct(p)
                                                if p.as_char() == ',' =>
                                            {
                                                if comma_encountered {
                                                    return Err(Error::PathAttrParseError {
                                                        module: mod_syn_path,
                                                        e: PathAttrParseError::CfgAttrNotTwoParams,
                                                    });
                                                }
                                                comma_encountered = true;
                                            }
                                            x => {
                                                if comma_encountered {
                                                    ts_after_comma.push(x)
                                                } else {
                                                    ts_before_comma.extend(std::iter::once(x))
                                                }
                                            }
                                        }
                                    }
                                    if !comma_encountered {
                                        return Err(Error::PathAttrParseError {
                                            module: mod_syn_path,
                                            e: PathAttrParseError::CfgAttrNotTwoParams,
                                        });
                                    }

                                    let mut pathy = false;
                                    if ! ts_after_comma.is_empty() {
                                        match &ts_after_comma[0] {
                                            TokenTree::Ident(i)if i.to_string()
                                            == "path" => pathy = true,
                                            _ => (),
                                        }
                                    }

                                    if pathy {
                                        path_attrs.push((ts_after_comma[1..].to_vec(), Some(ts_before_comma)));
                                    } else {
                                        attrs.push(attr.clone());
                                    }
                                }
                                _ => attrs.push(attr.clone()),
                            }
                        }

                        for (tt, cfg) in path_attrs {
                            if cfg.is_none() && inner.is_some() {
                                return Err(
                                    Error::MultipleExplicitPathsSpecifiedForOneModule {
                                        module: mod_syn_path,
                                    },
                                );
                            }

                            if tt.len() != 2 {
                                return Err(Error::PathAttrParseError {
                                    module: mod_syn_path,
                                    e: PathAttrParseError::NotExactlyTwoTokens,
                                });
                            }

                            match &tt[0] {
                                TokenTree::Punct(x) if x.as_char() == '=' => {
                                    ()
                                }
                                _ => {
                                    return Err(Error::PathAttrParseError {
                                        module: mod_syn_path,
                                        e: PathAttrParseError::FirstTokenIsNotEqualSign,
                                    })
                                }
                            }
                            match &tt[1] {
                                TokenTree::Literal(_) => (),
                                _ => return Err(Error::PathAttrParseError {
                                    module: mod_syn_path,
                                    e: PathAttrParseError::SecondTokenIsNotStringLiteral,
                                }),
                            }
                            let ts = TokenStream::from(tt[1].clone());
                            let tslit: syn::Lit =
                                syn::parse2(ts).map_err(|e| Error::SynParseError {
                                    module: mod_syn_path.clone(),
                                    e,
                                })?;
                            let explicit_path = match tslit {
                                syn::Lit::Str(x) => PathBuf::from(x.value()),
                                _ => return Err(Error::PathAttrParseError {
                                    module: mod_syn_path,
                                    e: PathAttrParseError::SecondTokenIsNotStringLiteral,
                                }),
                            };

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
                                let cfg: syn::Meta = syn::parse2(cfg).map_err(|e|Error::SynParseError{module:mod_syn_path.clone(),e})?;
                                if let Some(x) = resolver.resolve(
                                    mod_syn_path.clone(),
                                    module_file_explicit,
                                    Some(cfg),
                                )? {
                                    if inner.is_some() {
                                        return Err(
                                            Error::MultipleExplicitPathsSpecifiedForOneModule {
                                                module: mod_syn_path,
                                            },
                                        );
                                    }
                                    inner = Some(Some(x));
                                }
                            } else {
                                inner = Some(resolver.resolve(
                                    mod_syn_path.clone(),
                                    module_file_explicit,
                                    None,
                                )?);
                            }
                        }

                        let inner = if let Some(i) = inner {
                            dirs = dirs_candidate;
                            i
                        } else {
                            dirs.push_back(PathBuf::from(chunk));
                            let inner_nomod =
                                resolver.resolve(mod_syn_path.clone(), module_file_nomod, None);
                            match inner_nomod {
                                Ok(_) => (),
                                Err(Error::FailedToOpenFile { .. }) => (),
                                Err(e) => return Err(e),
                            }
                            let inner_mod = resolver.resolve(
                                mod_syn_path.clone(),
                                module_file_mod.clone(),
                                None,
                            );
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
                                (_, Ok(Some(x))) => {
                                    Some(x)
                                }
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
                                vis: m.vis.clone(),
                                mod_token: m.mod_token,
                                ident: id,
                                content: Some((syn::token::Brace(semi.span), items)),
                                semi: None,
                            };
                            replacement_item = Some(syn::Item::Mod(new_mod));
                        }
                    }
                }
            }
            _ => (),
        }
        if let Some(ri) = replacement_item {
            *item = ri;
        }
    }
    Ok(())
}