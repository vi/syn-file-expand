# syn-file-expand

This library allows you to load full source code of multi-file crates into a single [`syn::File`](https://docs.rs/syn/latest/syn/struct.File.html).

Features:

* Based on `syn` crate.
* Handling `#[path]` attributes
* Handling `#[cfg]` where it affects modules to filesystem mapping
* There is both a lower-level IO-less function and a simpler to use function.

Start exploring the library from the [`read_full_crate_source_code`] function.

# Bonus: CLI tool 

`synfileexpand` tool expands Rust sources, like `cargo expand`, but without macro expansion, only for modules.
`rustc` is not involved.

```text
$ synfileexpand src/lib.rs | rustfmt
# ! [doc = include_str ! ("../README.md")]
#![forbid(unsafe_code)]
...
mod attrs {
    use super::AttrParseError;
    ...
}
mod expand_impl {
    use crate::{attrs, Error, ErrorCase, Resolver};
    ...
}

$ synfileexpand
Usage: synfileexpand <Rust source file>
Reads rust source file, including referred modules and expands them into a single source with all modules inline
Conditional paths like #[cfg_attr(feature="qqq"),path=...) are resolved using
environment variables like SYN_FILE_EXPAND_FEATURE_QQQ=1
Other influential envvars: SYN_FILE_EXPAND_DEBUGVARS=1 SYN_FILE_EXPAND_DEFAULTTRUE=1
```

There is [a Github release](https://github.com/vi/syn-file-expand/releases/) with the tool pre-built for various platforms. To build the tool from source code, use `expand` example.
