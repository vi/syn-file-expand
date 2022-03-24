use proc_macro2;
use proc_macro2::TokenStream;

use super::PathAttrParseError;

use super::Error;

use std::path::PathBuf;

use proc_macro2::TokenTree;

pub(crate) fn extract_path_from_attr(
    tt: Vec<TokenTree>,
    mod_syn_path: &syn::Path,
) -> Result<PathBuf, Error> {
    if tt.len() != 2 {
        return Err(Error::PathAttrParseError {
            module: mod_syn_path.clone(),
            e: PathAttrParseError::NotExactlyTwoTokens,
        });
    }
    match &tt[0] {
        TokenTree::Punct(x) if x.as_char() == '=' => (),
        _ => {
            return Err(Error::PathAttrParseError {
                module: mod_syn_path.clone(),
                e: PathAttrParseError::FirstTokenIsNotEqualSign,
            })
        }
    }
    match &tt[1] {
        TokenTree::Literal(_) => (),
        _ => {
            return Err(Error::PathAttrParseError {
                module: mod_syn_path.clone(),
                e: PathAttrParseError::SecondTokenIsNotStringLiteral,
            })
        }
    }
    let ts = TokenStream::from(tt[1].clone());
    let tslit: syn::Lit = syn::parse2(ts).map_err(|e| Error::SynParseError {
        module: mod_syn_path.clone(),
        e,
    })?;
    let explicit_path = match tslit {
        syn::Lit::Str(x) => PathBuf::from(x.value()),
        _ => {
            return Err(Error::PathAttrParseError {
                module: mod_syn_path.clone(),
                e: PathAttrParseError::SecondTokenIsNotStringLiteral,
            })
        }
    };
    Ok(explicit_path)
}

pub(crate) fn extract_path_and_cfg_path_attrs(
    input_attrs: &[syn::Attribute],
    path_attrs: &mut Vec<(Vec<TokenTree>, Option<TokenStream>)>,
    module_name: &syn::Path,
    attrs: &mut Vec<syn::Attribute>,
) -> Result<(), Error> {
    for attr in input_attrs {
        match &attr.path {
            x if x.get_ident().map(|x| x.to_string()) == Some("path".to_owned()) => {
                let tt = Vec::<TokenTree>::from_iter(attr.tokens.clone());
                path_attrs.push((tt, None));
            }
            x if x.get_ident().map(|x| x.to_string()) == Some("cfg_attr".to_owned()) => {
                let tt = Vec::<TokenTree>::from_iter(attr.tokens.clone());
                if tt.len() != 1 {
                    return Err(Error::PathAttrParseError {
                        module: module_name.clone(),
                        e: PathAttrParseError::CfgAttrNotRoundGroup,
                    });
                }
                let t = tt.into_iter().next().unwrap();
                let g = match t {
                    TokenTree::Group(g)
                        if g.delimiter() == proc_macro2::Delimiter::Parenthesis =>
                    {
                        g
                    }
                    _ => {
                        return Err(Error::PathAttrParseError {
                            module: module_name.clone(),
                            e: PathAttrParseError::CfgAttrNotRoundGroup,
                        })
                    }
                };
                let inner = Vec::<TokenTree>::from_iter(g.stream());
                if inner.len() < 3 {
                    return Err(Error::PathAttrParseError {
                        module: module_name.clone(),
                        e: PathAttrParseError::CfgAttrNotTwoParams,
                    });
                }
                let mut ts_before_comma = TokenStream::new();
                let mut ts_after_comma = Vec::<TokenTree>::new();
                let mut comma_encountered = false;
                for t in inner.into_iter() {
                    match t {
                        TokenTree::Punct(p) if p.as_char() == ',' => {
                            if comma_encountered {
                                return Err(Error::PathAttrParseError {
                                    module: module_name.clone(),
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
                        module: module_name.clone(),
                        e: PathAttrParseError::CfgAttrNotTwoParams,
                    });
                }

                let mut pathy = false;
                if !ts_after_comma.is_empty() {
                    match &ts_after_comma[0] {
                        TokenTree::Ident(i) if i.to_string() == "path" => pathy = true,
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
    Ok(())
}
