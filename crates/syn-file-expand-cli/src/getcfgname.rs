
use quote::ToTokens;
pub(crate) fn get_env_name(input: proc_macro2::TokenStream) -> String {
    let mut buf = String::new();
    let mut need_underscore = false;
    for t in input {
        let to_insert: String = match t {
            proc_macro2::TokenTree::Group(g) => {
                let ret = get_env_name(g.stream());
                ret
            }
            proc_macro2::TokenTree::Ident(x) => {
                let x = x.to_string().to_ascii_uppercase();
                x
            }
            proc_macro2::TokenTree::Literal(x) => {
                if let Ok(l) = syn::parse2::<syn::Lit>(x.clone().into_token_stream()) {
                    match l {
                        syn::Lit::Str(x) => x
                            .value()
                            .to_ascii_uppercase()
                            .replace(" ", "_")
                            .replace("-", "_"),
                        syn::Lit::ByteStr(_) => "".to_owned(),
                        syn::Lit::Byte(_) => "".to_owned(),
                        syn::Lit::Char(x) => format!("{}", x.value()),
                        syn::Lit::Int(x) => x.to_string(),
                        syn::Lit::Float(_) => "".to_owned(),
                        syn::Lit::Bool(x) => format!("{}", x.value),
                        syn::Lit::Verbatim(_) => "".to_owned(),
                    }
                } else {
                    eprintln!("Failed to parse a literal in a cfg `{}`", x);
                    "".to_owned()
                }
            }
            proc_macro2::TokenTree::Punct(_) => "".to_owned(),
        };
        if !to_insert.is_empty() {
            if need_underscore {
                buf += "_";
            }
            buf += &to_insert;
            need_underscore = true;
        }
    }
    buf
}

pub(crate) fn get_cli_name(input: proc_macro2::TokenStream) -> String {
    let mut buf = String::new();
    for t in input {
        let to_insert: String = match t {
            proc_macro2::TokenTree::Group(g) => {
                let ret = get_cli_name(g.stream());
                match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => format!("({})", ret),
                    proc_macro2::Delimiter::Brace => format!("{{{}}}", ret),
                    proc_macro2::Delimiter::Bracket => format!("[{}]", ret),
                    proc_macro2::Delimiter::None => format!("{}", ret),
                }
            }
            proc_macro2::TokenTree::Ident(x) => {
                let x = x.to_string();
                x
            }
            proc_macro2::TokenTree::Literal(x) => {
                if let Ok(l) = syn::parse2::<syn::Lit>(x.clone().into_token_stream()) {
                    match l {
                        syn::Lit::Str(x) => x.value(),
                        syn::Lit::ByteStr(_) => "".to_owned(),
                        syn::Lit::Byte(_) => "".to_owned(),
                        syn::Lit::Char(x) => format!("{}", x.value()),
                        syn::Lit::Int(x) => x.to_string(),
                        syn::Lit::Float(_) => "".to_owned(),
                        syn::Lit::Bool(x) => format!("{}", x.value),
                        syn::Lit::Verbatim(_) => "".to_owned(),
                    }
                } else {
                    eprintln!("Failed to parse a literal in a cfg `{}`", x);
                    "".to_owned()
                }
            }
            proc_macro2::TokenTree::Punct(p) => format!("{}", p.as_char()),
        };
        buf += &to_insert;
    }
    buf
}
