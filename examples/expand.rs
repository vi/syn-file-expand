use std::{ffi::OsString};

use quote::ToTokens;

fn main() -> Result<(), syn_file_expand::Error> {
    let args = Vec::<OsString>::from_iter(std::env::args_os());
    if args.len() != 2 {
        println!("Usage: expand <Rust source file>");
        println!("Reads rust source file, including referred modules and expands them into a single source with all modules inline");
        println!("All #[cfg]s are assumed to be false. See `syn-file-expand-cli` crate for extended version of this tool.");
        std::process::exit(1);
    }
    
    let source = syn_file_expand::read_full_crate_source_code(&args[1], |_|Ok(false))?;
    println!("{}", source.into_token_stream());
    Ok(())
}
