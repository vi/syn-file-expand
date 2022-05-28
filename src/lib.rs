#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::PathBuf;
use thiserror::Error;

pub extern crate proc_macro2;
pub extern crate syn;

use im_rc::Vector;

/// Generic error type to report from callbacks
pub type UserError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Error originated from parsing of `#[path]`, `#[cfg]` or `#[cfg_attr]` attibutes
/// in code contained in this crate.
/// See `Display` implementation for details about specific variants.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum AttrParseError {
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
    #[error("`#[cfg` is not followed by a sole round parentheses group")]
    MalformedCfg,
}

/// Specifics of error when expanding a particular module
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum ErrorCase {
    #[error("Cannot open file {path}: {e}")]
    FailedToOpenFile { path: PathBuf, e: std::io::Error },
    #[error("The module has multiple explicit #[path] directives")]
    MultipleExplicitPathsSpecifiedForOneModule,
    #[error("Both name/mod.rs and name.rs present")]
    BothModRsAndNameRsPresent,
    #[error("error parsing attribute: {0}")]
    AttrParseError(AttrParseError),
    #[error("syn parsing error: {0}")]
    SynParseError(syn::parse::Error),
    #[error("Error from callback: {0}")]
    ErrorFromCallback(UserError),
}

/// Main error type that is returned from functions of this crate, as well as from some user callbacks.
#[derive(Error, Debug)]
#[error("Expanding module `{module}`: {inner}", module=PathForDisplay(module))]
pub struct Error {
    /// Module being expanded. Root module is represented by an empty path.
    pub module: syn::Path,
    /// Specific error
    pub inner: ErrorCase,
}

struct PathForDisplay<'a>(&'a syn::Path);
impl<'a> std::fmt::Display for PathForDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        itertools::join(self.0.segments.iter().map(|x| x.ident.to_string()), "::").fmt(f)
    }
}

/// Data and configuration source for the [`expand_modules_into_inline_modules`] function.
/// You can use [`ResolverHelper`] instead of manually implementing this.
pub trait Resolver {
    /// Called each time a non-inline module is encountered that needs to be expanded.
    /// `module_name` is full path from the specified file root to the module being expanded,
    /// to use for error messages.
    ///
    /// `relative_path` is pathname of a file the module should be expanded from,
    /// relative to root module you are expanding. You will probably need to join this path
    /// to the path of `src` directory to open the actual file.
    ///
    /// This callback may be called multiple times (e.g. for `mymodule.rs` and `mymodule/mod.rs`).
    /// It is error to return more than one Ok(Some) for one module, unless `allow_duplicate_modules_and_convert_cfg` is set to true.
    ///
    /// Retuning Ok(None) for all candidate files leaves the module unexpanded in [`expand_modules_into_inline_modules`]'s output.
    fn resolve(
        &mut self,
        module_name: syn::Path,
        path_relative_to_crate_root: PathBuf,
    ) -> Result<Option<syn::File>, Error>;

    /// When `#[cfg(mymeta)] mod ...;` or `#[cfg_attr(mymeta,path=...)]` is encountered, this function is called
    /// and you should provide answer whether this cfg should be considered true or false.
    fn check_cfg(&mut self, cfg: syn::Meta) -> Result<bool, UserError>;

    /// Include all the modules, possibly duplicating them.
    /// `#[cfg_attr(...,path)] mod ...;` are converted to `#[cfg(...)] mod .. {}`
    /// 
    /// `check_cfg` is still called, but it is not a problem to return `true` unconditionally.
    fn allow_duplicate_modules_and_convert_cfg(&mut self) -> bool { false }
}

/// Helper struct to define `Resolver` implementations using closures.
pub struct ResolverHelper<F1, F2>(pub F1, pub F2)
where
    F1: FnMut(syn::Path, PathBuf) -> Result<Option<syn::File>, Error>,
    F2: FnMut(syn::Meta) -> Result<bool, UserError>;
impl<F1, F2> Resolver for ResolverHelper<F1, F2>
where
    F1: FnMut(syn::Path, PathBuf) -> Result<Option<syn::File>, Error>,
    F2: FnMut(syn::Meta) -> Result<bool, UserError>,
{
    fn resolve(
        &mut self,
        module_name: syn::Path,
        path_relative_to_crate_root: PathBuf,
    ) -> Result<Option<syn::File>, Error> {
        (self.0)(module_name, path_relative_to_crate_root)
    }

    fn check_cfg(&mut self, cfg: syn::Meta) -> Result<bool, UserError> {
        (self.1)(cfg)
    }
}

/// Take a `syn` representation of a Rust source file and turn into similar `syn` representation,
/// but with `mod something;` expanded into `mod something { ... }`.
///
/// It is low-level IO-agnostic function: your callback is responsible for reading and parsing the data.
///
/// Use [`read_full_crate_source_code`] for easy way to just load crate source code.
///
/// Example:
///
/// ```
/// # fn main() -> Result<(), syn_file_expand::Error> {
/// let mut ast: syn::File = syn::parse2(quote::quote! {
///     mod inner_module;
/// }).unwrap();
/// let cfg_evaluator = |_cfg: syn::Meta|Ok(false);
/// let code_loader = |_module:syn::Path, path:std::path::PathBuf|{
///    if path == std::path::Path::new("inner_module/mod.rs") {
///        Ok(Some(syn::parse2(quote::quote! {
///            trait Foo {
///            }
///        }).unwrap()))
///    } else {
///        Ok(None)
///    }
/// };
/// let mut resolver = syn_file_expand::ResolverHelper(code_loader, cfg_evaluator);
/// syn_file_expand::expand_modules_into_inline_modules(&mut ast, &mut resolver)?;
///
/// let expanded: syn::File = syn::parse2(quote::quote! {
///     mod inner_module {
///         trait Foo { }
///     }
/// }).unwrap();
///
/// assert_eq!(ast, expanded);
/// #   Ok(())
/// # }
/// ```
pub fn expand_modules_into_inline_modules<R: Resolver>(
    content: &mut syn::File,
    resolver: &mut R,
) -> Result<(), Error> {
    let dirs = Vector::new();
    let mutltimodule_mode = resolver.allow_duplicate_modules_and_convert_cfg();
    expand_impl::expand_impl(
        &mut content.items,
        resolver,
        Vector::new(),
        dirs.clone(),
        dirs,
        mutltimodule_mode,
    )?;
    Ok(())
}

/// Easy function to load full crate source code from the filesystem.
///
/// If `#[cfg_attr(some_meta,path=...)]` is encountered while reading modules, your callback
/// predicate `cfg_attr_path_handler` is called with `some_meta` and should decide whether to
/// follow this cfg_attr or not.
///
/// Just specify `|_|Ok(false)` there if you are confused.
///
/// **Security**: Note that content of file being loaded may point to arbitrary file, including using absolute paths.
/// Use IO-less [`expand_modules_into_inline_modules`] function if you want to control what is allowed to be read.
/// Also `#[path]` of a module may point to the same file, leading to endless recursion, exausting memory and/or stack.
///
/// Example:
///
/// ```
/// # fn main() -> Result<(), syn_file_expand::Error> {
/// let mut input_file = std::path::PathBuf::new();
/// # input_file.push(env!("CARGO_MANIFEST_DIR"));
/// input_file.push("src");
/// input_file.push("lib.rs");
/// let ast : syn::File = syn_file_expand::read_full_crate_source_code(input_file, |_|Ok(false))?;
/// assert!(ast.items.iter()
///    .filter_map(|x|match x { syn::Item::Fn(y) => Some(y), _ => None})
///    .map(|x|x.sig.ident.to_string())
///    .find(|x|x == "read_full_crate_source_code") != None
/// );
/// #   Ok(())
/// # }
/// ```
pub fn read_full_crate_source_code(
    path: impl AsRef<std::path::Path>,
    cfg_attr_path_handler: impl FnMut(syn::Meta) -> Result<bool, UserError>,
) -> Result<syn::File, Error> {
    let path = path.as_ref();
    let root_source = std::fs::read_to_string(path).map_err(|e| Error {
        module: syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::new(),
        },
        inner: ErrorCase::FailedToOpenFile {
            path: path.to_owned(),
            e,
        },
    })?;
    let mut root_source: syn::File = syn::parse_file(&root_source).map_err(|e| Error {
        module: syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::new(),
        },
        inner: ErrorCase::SynParseError(e),
    })?;

    let parent_dir = path.parent();

    struct MyResolver<'a, F: FnMut(syn::Meta) -> Result<bool, UserError>> {
        cfg_attr_path_handler: F,
        parent_dir: Option<&'a std::path::Path>,
    }

    impl<'a, F: FnMut(syn::Meta) -> Result<bool, UserError>> Resolver for MyResolver<'a, F> {
        fn resolve(
            &mut self,
            module_name: syn::Path,
            path_relative_to_crate_root: PathBuf,
        ) -> Result<Option<syn::File>, Error> {
            let path = if let Some(parent_dir) = self.parent_dir {
                parent_dir.join(&path_relative_to_crate_root)
            } else {
                path_relative_to_crate_root
            };
            let module_source = std::fs::read_to_string(&path).map_err(|e| Error {
                module: module_name.clone(),
                inner: ErrorCase::FailedToOpenFile {
                    path: path.clone(),
                    e,
                },
            })?;
            let module_source: syn::File = syn::parse_file(&module_source).map_err(|e| Error {
                module: module_name,
                inner: ErrorCase::SynParseError(e),
            })?;
            Ok(Some(module_source))
        }

        fn check_cfg(&mut self, cfg: syn::Meta) -> Result<bool, UserError> {
            (self.cfg_attr_path_handler)(cfg)
        }
    }

    expand_modules_into_inline_modules(
        &mut root_source,
        &mut MyResolver {
            cfg_attr_path_handler,
            parent_dir,
        },
    )?;
    Ok(root_source)
}

/// Even easier function to load full crate source code from the filesystem.
/// 
/// Duplicates modules in case of multiple variants cause by cfg attributes.
/// 
/// See the warnings in [`read_full_crate_source_code`] documentation.
pub fn read_full_crate_source_code_with_dupes(
    path: impl AsRef<std::path::Path>,
) -> Result<syn::File, Error> {
    let path = path.as_ref();
    let root_source = std::fs::read_to_string(path).map_err(|e| Error {
        module: syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::new(),
        },
        inner: ErrorCase::FailedToOpenFile {
            path: path.to_owned(),
            e,
        },
    })?;
    let mut root_source: syn::File = syn::parse_file(&root_source).map_err(|e| Error {
        module: syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::new(),
        },
        inner: ErrorCase::SynParseError(e),
    })?;

    let parent_dir = path.parent();

    struct MyResolver2<'a> {
        parent_dir: Option<&'a std::path::Path>,
    }

    impl<'a> Resolver for MyResolver2<'a> {
        fn resolve(
            &mut self,
            module_name: syn::Path,
            path_relative_to_crate_root: PathBuf,
        ) -> Result<Option<syn::File>, Error> {
            let path = if let Some(parent_dir) = self.parent_dir {
                parent_dir.join(&path_relative_to_crate_root)
            } else {
                path_relative_to_crate_root
            };
            let module_source = std::fs::read_to_string(&path).map_err(|e| Error {
                module: module_name.clone(),
                inner: ErrorCase::FailedToOpenFile {
                    path: path.clone(),
                    e,
                },
            })?;
            let module_source: syn::File = syn::parse_file(&module_source).map_err(|e| Error {
                module: module_name,
                inner: ErrorCase::SynParseError(e),
            })?;
            Ok(Some(module_source))
        }

        fn check_cfg(&mut self, _cfg: syn::Meta) -> Result<bool, UserError> {
            Ok(true)
        }

        fn allow_duplicate_modules_and_convert_cfg(&mut self) -> bool {
            true
        }
    }

    expand_modules_into_inline_modules(
        &mut root_source,
        &mut MyResolver2 {
            parent_dir,
        },
    )?;
    Ok(root_source)
}

mod attrs;
mod expand_impl;
