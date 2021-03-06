use std::{collections::HashSet, path::PathBuf};

use quote::ToTokens;

/// Use `syn-file-expand-cli -fTp src/lib.rs` as a starting point.
/// 
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

    /** Convert all blocks and expressions to `loop{}`s.
                      Note that inner items within blocks get lost in the process.*/
    #[options(short = 'l')]
    loopify: bool,

    /// Strip all documentation attributes. Note that inner items within blocks are not processed and may retain their attributes.
    #[options(short = 'D')]
    undoc: bool,

    /// Assume all `#[cfg]`s and `#[cfg_attr]`s are true. May lead to errors unless `-f` is also used.
    #[options(short = 'T')]
    cfg_true_by_default: bool,

    /// Allow duplicate modules, also preserve/transform some `cfg` attributes.
    /// With `-T` it allows to read the full crate source code for all cfg variants.
    #[options(short = 'f')]
    full_crate_tree: bool,


    /** Set this cfg check result to true.
                                Note that `all` or `any` are not handled.
                                You need to set all needed expression results one by one.
                                Strings required for --cfg are not the same as for environment variables-
                                -based version of this feature.*/
    #[options(short = 'c')]
    cfg: Vec<String>,

    /// In `--cfg-true-by-default` mode, explicitly unset given cfg expression outcome.
    #[options(short = 'u')]
    unset_cfg: Vec<String>,

    /** Print each encountered cfg check to stderr, in form suitable for `--cfg` parameter
                   Note that the format is different from the one used by `SYN_FILE_EXPAND_DEBUGVARS=1` environment variable.*/
    #[options(short = 'd')]
    debug_cfg: bool,

    /// Use given file for output instead of stdout
    #[options(short = 'o')]
    output: Option<PathBuf>,

    /// Use `prettyplease` to format the output
    #[options(short = 'p')]
    pretty: bool,
}

mod getcfgname;
mod loopify;
mod undoc;

fn main() {
    let opts: Opts = gumdrop::parse_args_or_exit(gumdrop::ParsingStyle::AllOptions);

    let set_cfg = HashSet::<String>::from_iter(opts.cfg.into_iter());
    let unset_cfg = HashSet::<String>::from_iter(opts.unset_cfg.into_iter());

    let debug_env = std::env::var("SYN_FILE_EXPAND_DEBUGVARS") == Ok("1".to_owned());
    let default = std::env::var("SYN_FILE_EXPAND_DEFAULTTRUE") == Ok("1".to_owned());
    let mut source = match syn_file_expand::read_full_crate_source_code_ex(&opts.input_file, |cfg| {
        let envname = format!(
            "SYN_FILE_EXPAND_{}",
            getcfgname::get_env_name(cfg.clone().to_token_stream())
        );
        let cliname = getcfgname::get_cli_name(cfg.to_token_stream());
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
    }, opts.full_crate_tree) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(2)
        }
    };
    if opts.loopify {
        loopify::loopify(&mut source);
    }
    if opts.undoc {
        undoc::undoc(&mut source);
    }
    if let Some(output) = opts.output {
        match (|| -> std::io::Result<()> {
            use std::io::Write;
            let f = std::fs::File::create(output)?;
            let mut f = std::io::BufWriter::with_capacity(128*1024, f);
            if opts.pretty {
                #[cfg(feature="prettyplease")]
                writeln!(f, "{}", prettyplease::unparse(&source))?;
                #[cfg(not(feature="prettyplease"))]
                {
                    eprintln!("--pretty support is not enabled at compile time");
                    std::process::exit(3);
                } 
            } else {
                writeln!(f, "{}", source.into_token_stream())?;
            }
            Ok(())
        })() {
            Ok(()) => (),
            Err(e) => {
                eprintln!("Output failed: {}", e);
                std::process::exit(3)
            }
        }
    } else {
        if opts.pretty {
            #[cfg(feature="prettyplease")]
            println!("{}", prettyplease::unparse(&source));
            #[cfg(not(feature="prettyplease"))]
            {
                eprintln!("--pretty support is not enabled at compile time");
                std::process::exit(3);
            } 
        } else {
            println!("{}", source.into_token_stream());
        }
    }
}
