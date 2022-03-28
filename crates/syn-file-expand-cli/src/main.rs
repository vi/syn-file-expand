use std::{collections::HashSet, path::PathBuf};

use quote::ToTokens;

/// Reads rust source file, including referred modules and expands them into a single source with all modules inline
/// Apart from respective dedicated command line arguments, conditional paths like
/// `#[cfg_attr(feature="qqq"),path=...)` are resolved using
/// environment variables like SYN_FILE_EXPAND_FEATURE_QQQ=1
/// Other influential envvars: SYN_FILE_EXPAND_DEBUGVARS=1 SYN_FILE_EXPAND_DEFAULTTRUE=1
#[derive(gumdrop::Options)]
struct Opts {
    help: bool,

    /// Input Rust source file to start crawling from
    #[options(free, required)]
    input_file: PathBuf,

    /// Convert all function bodies to `loop{}`s.
    #[options(short = 'l')]
    loopify: bool,

    /// Assume all `#[cfg]`s and `#[cfg_attr]`s are true.
    /// May lead to errors
    #[options(short = 'T')]
    cfg_true_by_default: bool,

    /// Set this cfg check result to true.
    /// Note that `all` or `any` are not handled.
    /// You need to set all needed expression results one by one.
    /// Note that much less processing happens
    /// to make prepare cfg expression for CLI usage compare to environment variable usage.
    #[options(short = 'c')]
    cfg: Vec<String>,

    /// In `--cfg-true-by-default` mode, explicitly unset given cfg expression outcome.
    #[options(short = 'u')]
    unset_cfg: Vec<String>,

    /// Print each encountered cfg check to stderr, in form suitable for `--cfg` parameter.
    /// Note that the format is different from the one used by `SYN_FILE_EXPAND_DEBUGVARS=1` environment variable.
    #[options(short = 'd')]
    debug_cfg: bool,
}

fn get_env_name(input: proc_macro2::TokenStream) -> String {
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
fn get_cli_name(input: proc_macro2::TokenStream) -> String {
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

fn main() {
    let opts: Opts = gumdrop::parse_args_or_exit(gumdrop::ParsingStyle::AllOptions);

    let set_cfg = HashSet::<String>::from_iter(opts.cfg.into_iter());
    let unset_cfg = HashSet::<String>::from_iter(opts.unset_cfg.into_iter());

    let debug_env = std::env::var("SYN_FILE_EXPAND_DEBUGVARS") == Ok("1".to_owned());
    let default = std::env::var("SYN_FILE_EXPAND_DEFAULTTRUE") == Ok("1".to_owned());
    let source = match syn_file_expand::read_full_crate_source_code(&opts.input_file, |cfg| {
        let envname = format!(
            "SYN_FILE_EXPAND_{}",
            get_env_name(cfg.clone().to_token_stream())
        );
        let cliname = get_cli_name(cfg.to_token_stream());
        if debug_env {
            eprintln!("{}", envname);
        }
        if opts.debug_cfg {
            eprintln!("{}", cliname);
        }
        Ok(if opts.cfg_true_by_default || set_cfg.contains(&cliname) {
            !unset_cfg.contains(&cliname)
        } else if let Ok(x) = std::env::var(&envname) {
            if x == "1" {
                true
            } else {
                false
            }
        } else {
            default
        })
    }) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(2)
        }
    };
    println!("{}", source.into_token_stream());
}
