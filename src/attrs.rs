use proc_macro2;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::MacroDelimiter;
use syn::Meta;
use syn::MetaList;
use syn::MetaNameValue;
use syn::Token;
use syn::punctuated::Punctuated;


use super::AttrParseError;


use std::path::PathBuf;


pub(crate) fn read_and_process_attributes(
    input_attrs: &[syn::Attribute],
    path_attrs: &mut Vec<(PathBuf, Option<TokenStream>)>,
    attrs: &mut Vec<syn::Attribute>,
    cfg_attrs: &mut Vec<TokenStream>,
) -> Result<(), AttrParseError> {
    for attr in input_attrs {
        match &attr.meta {
            Meta::List(MetaList { path, delimiter, tokens }) if path.is_ident("cfg") => {
                if !matches!(delimiter, MacroDelimiter::Paren(..)) {
                    return Err(AttrParseError::MalformedCfg);
                }
                cfg_attrs.push(tokens.clone());
            }
            Meta::NameValue(MetaNameValue { path, eq_token: _, value }) if path.is_ident("path") => {
                let path = match value {
                    syn::Expr::Lit(lit) => match &lit.lit {
                        syn::Lit::Str(x) => x.value(),
                        _ => return Err(AttrParseError::SecondTokenIsNotStringLiteral),
                    }
                    _ => return Err(AttrParseError::SecondTokenIsNotStringLiteral),
                };
                path_attrs.push((PathBuf::from(path), None));
            }
            Meta::Path(path) | Meta::List(MetaList { path, .. }) if path.is_ident("path") => {
                return Err(AttrParseError::FirstTokenIsNotEqualSign);
            }
            
            Meta::Path(path) | Meta::NameValue(MetaNameValue { path, .. }) if path.is_ident("cfg_attr") => {
                return Err(AttrParseError::CfgAttrNotRoundGroup);
            }

            Meta::List(MetaList { path, delimiter, .. }) if path.is_ident("cfg_attr") => {
                if !matches!(delimiter, MacroDelimiter::Paren(..)) {
                    return Err(AttrParseError::CfgAttrNotRoundGroup);
                }

                let Ok(nested) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
                    return Err(AttrParseError::MalformedCfg)
                };

                if nested.len() != 2 {
                    return Err(AttrParseError::CfgAttrNotTwoParams);
                }

                let condition = &nested[0];
                let potential_path = &nested[1];

                if ! potential_path.path().is_ident("path") {
                    attrs.push(attr.clone());
                    continue;
                }

                let path = match potential_path {
                    Meta::NameValue(MetaNameValue { path: _, eq_token: _, value }) => {
                        match value {
                            syn::Expr::Lit(lit) => match &lit.lit {
                                syn::Lit::Str(x) => x.value(),
                                _ => return Err(AttrParseError::SecondTokenIsNotStringLiteral),
                            }
                            _ => return Err(AttrParseError::SecondTokenIsNotStringLiteral),
                        }
                    }
                    _ => return Err(AttrParseError::FirstTokenIsNotEqualSign),
                };

                path_attrs.push((PathBuf::from(path), Some(condition.to_token_stream())));
            }

            _ => attrs.push(attr.clone()),
        }
    }
    Ok(())
}
