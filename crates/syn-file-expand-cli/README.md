# syn-file-expand-cli

`syn-file-expand-cli` tool expands Rust sources, like `cargo expand`, but without macro expansion, only for modules.
`rustc` is not involved.  Filtering through `rustfmt` is adviced for debugging use case. 

```text
$ syn-file-expand-cli src/lib.rs | rustfmt
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

$ syn-file-expand-cli --help
Usage: target/q/debug/syn-file-expand-cli [OPTIONS]

Reads rust source file, including referred modules and expands them into a single source with all modules inline
Apart from respective dedicated command line arguments, conditional paths like
`#[cfg_attr(feature="qqq"),path=...)` are resolved using
environment variables like SYN_FILE_EXPAND_FEATURE_QQQ=1
Other influential envvars: SYN_FILE_EXPAND_DEBUGVARS=1 SYN_FILE_EXPAND_DEFAULTTRUE=1

Positional arguments:
  input_file                 Input Rust source file to start crawling from

Optional arguments:
  -h, --help
  -l, --loopify              Convert all blocks and expressions to `loop{}`s.
                      Note that inner items within blocks get lost in the process.
  -D, --undoc                Stip all documentation attributes. Note that inner items within blocks are not processed and may retain their attributes.
  -T, --cfg-true-by-default  Assume all `#[cfg]`s and `#[cfg_attr]`s are true. May lead to errors
  -c, --cfg CFG              Set this cfg check result to true.
                                Note that `all` or `any` are not handled.
                                You need to set all needed expression results one by one.
                                Note that much less processing happens
                                to make prepare cfg expression for CLI usage compare to environment variable usage.
  -u, --unset-cfg UNSET-CFG  In `--cfg-true-by-default` mode, explicitly unset given cfg expression outcome.
  -d, --debug-cfg            Print each encountered cfg check to stderr, in form suitable for `--cfg` parameter
                   Note that the format is different from the one used by `SYN_FILE_EXPAND_DEBUGVARS=1` environment variable.
  -o, --output OUTPUT        Use given file for output instead of stdout
```

There are [Github releases](https://github.com/vi/syn-file-expand/releases/) with the tool pre-built for various platforms.  
You can also install the tool using `cargo install syn-file-expand-cli`.
