use std::{ffi::OsString};

use quote::ToTokens;

fn get_env_name(input: proc_macro2::TokenStream) -> String {
    let mut buf = String::new();
    let mut need_underscore = false;
    for t in input {
        let to_insert : String= match t {
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
                        syn::Lit::Str(x) => x.value().to_ascii_uppercase().replace(" ", "_").replace("-","_"),
                        syn::Lit::ByteStr(_) => "".to_owned(),
                        syn::Lit::Byte(_) => "".to_owned(),
                        syn::Lit::Char(x) => format!("{}", x.value()),
                        syn::Lit::Int(x) => x.to_string(),
                        syn::Lit::Float(_) => "".to_owned(),
                        syn::Lit::Bool(x) => format!("{}",x.value),
                        syn::Lit::Verbatim(_)=> "".to_owned(),
                    }
                } else {
                    eprintln!("Failed to parse a literal in a cfg `{}`", x);
                    "".to_owned()
                }
            }
            proc_macro2::TokenTree::Punct(_) => "".to_owned(),
        };
        if ! to_insert.is_empty() {
            if need_underscore {
                buf += "_";
            }
            buf += &to_insert;
            need_underscore = true;
        }
    }
    buf
}

fn main() -> Result<(), syn_file_expand::Error> {
    let args = Vec::<OsString>::from_iter(std::env::args_os());
    if args.len() != 2 {
        println!("Usage: syn-file-expand-cli <Rust source file>");
        println!("Reads rust source file, including referred modules and expands them into a single source with all modules inline");
        println!("Conditional paths like #[cfg_attr(feature=\"qqq\"),path=...) are resolved using");
        println!("environment variables like SYN_FILE_EXPAND_FEATURE_QQQ=1");
        println!("Other influential envvars: SYN_FILE_EXPAND_DEBUGVARS=1 SYN_FILE_EXPAND_DEFAULTTRUE=1");
        std::process::exit(1);
    }

    let debug_env = std::env::var("SYN_FILE_EXPAND_DEBUGVARS") == Ok("1".to_owned());
    let default = std::env::var("SYN_FILE_EXPAND_DEFAULTTRUE") == Ok("1".to_owned());
    let source = syn_file_expand::read_full_crate_source_code(&args[1], |cfg| {
        let envname = format!("SYN_FILE_EXPAND_{}", get_env_name(cfg.to_token_stream()));
        if debug_env {
            eprintln!("{}", envname);
        }
        Ok(if let Ok(x) = std::env::var(&envname) {
            if x == "1" {
                true
            } else {
                false
            }
        } else {
            default
        })
    })?;
    println!("{}", source.into_token_stream());
    Ok(())
}
