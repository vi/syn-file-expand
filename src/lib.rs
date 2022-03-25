use std::path::PathBuf;
use thiserror::Error;

pub extern crate proc_macro2;
pub extern crate syn;

use im_rc::Vector;

pub type UserError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Error, Debug)]
pub enum PathAttrParseError {
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
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Cannot open file {path}: {e}")]
    FailedToOpenFile { path: PathBuf, e: std::io::Error },
    #[error("The module have multiple explicit #[path] directives for {module}", module=PathForDisplay(module))]
    MultipleExplicitPathsSpecifiedForOneModule { module: syn::Path },
    #[error("Both name/mod.rs and name.rs present for {module}", module=PathForDisplay(module))]
    BothModRsAndNameRsPresent { module: syn::Path },
    #[error("error parsing path or cfg_attr path attribute in {module}: {e}", module=PathForDisplay(module))]
    PathAttrParseError {
        module: syn::Path,
        e: PathAttrParseError,
    },
    #[error("parsing error: {e} in {module}", module=PathForDisplay(module))]
    SynParseError {
        module: syn::Path,
        e: syn::parse::Error,
    },
    #[error("Error from callback: {e}")]
    ErrorFromCallback { e: UserError },
}

struct PathForDisplay<'a>(&'a syn::Path);
impl<'a> std::fmt::Display for PathForDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        itertools::join(self.0.segments.iter().map(|x| x.ident.to_string()), "::").fmt(f)
    }
}

/// Data and configuration source for the progress of expansion.
pub trait Resolver {
    /// Called by `expand_modules_into_inline_modules` each time a non-inline module is encountered.
    /// `module_name` is full path from the specified file root to the module being expanded,
    /// separated by `::`, including the module name of the file being expanded.
    ///
    /// `relative_path` is pathname of a file the module should be expanded from,
    /// relative to root module you are expanding.
    ///
    /// `relative_path_mod` is alternative location of the file, for `mod.rs` variant.
    ///
    /// `cfg` is x from `#[cfg_attr(x, path=...)]` or None if path is not overridden or unconditionally overridden.
    ///
    /// This callback may be called multiple times if there are multiple `#[path]` or `#[cfg_attr(...,path)]` attributes.
    /// It is error to return more than one Ok(Some) for one module.
    ///
    /// Retuning Ok(None) for all potential files leaves the module unexpanded in `expand_modules_into_inline_modules` output.
    fn resolve(
        &mut self,
        module_name: syn::Path,
        path_relative_to_crate_root: PathBuf,
    ) -> Result<Option<syn::File>, Error>;

    /// When `#[cfg(mymeta)] mod ...;` or `#[cfg_attr(mymeta,path=...)]` is encountered, this function is called
    /// and you should provide answer whether this cfg should be considered true or false.
    fn check_cfg(&mut self, cfg: syn::Meta) -> Result<bool, UserError>;
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
pub fn expand_modules_into_inline_modules<R: Resolver>(
    content: &mut syn::File,
    resolver: &mut R,
) -> Result<(), Error> {
    let dirs = Vector::new();
    expand_impl::expand_impl(
        &mut content.items,
        resolver,
        Vector::new(),
        dirs.clone(),
        dirs,
    )?;
    Ok(())
}

/// Easy function to load full crate source code from the filesystem.
///
/// If `#[cfg_attr(some_meta,path=...)]` is encountered while reading modules, your callback
/// predicate `cfg_attr_path_handler` is called with `some_meta` and should decide whether to
/// follow this cfg_attr or not
///
/// Just specify `|_|Ok(false)` there if you are confused.
///
/// Security: Note that content of file being loaded may point to arbitrary file, including using absolute paths.
/// Use IO-less `expand_modules_into_inline_modules` function if you want to control what is allowed to be read.
pub fn read_full_crate_source_code(
    path: impl AsRef<std::path::Path>,
    cfg_attr_path_handler: impl FnMut(syn::Meta) -> Result<bool, UserError>,
) -> Result<syn::File, Error> {
    let path = path.as_ref();
    let root_source = std::fs::read_to_string(path).map_err(|e| Error::FailedToOpenFile {
        path: path.to_owned(),
        e,
    })?;
    let mut root_source: syn::File =
        syn::parse_file(&root_source).map_err(|e| Error::SynParseError {
            module: syn::Path {
                leading_colon: None,
                segments: syn::punctuated::Punctuated::new(),
            },
            e,
        })?;

    let parent_dir = path.parent();

    struct MyResolver<'a, F: FnMut(syn::Meta) -> Result<bool, UserError>> {
        cfg_attr_path_handler: F,
        parent_dir: Option<&'a std::path::Path>,
    }

    impl<'a, F: FnMut(syn::Meta) -> Result<bool,UserError>> Resolver for MyResolver<'a, F> {
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
            let module_source =
                std::fs::read_to_string(&path).map_err(|e| Error::FailedToOpenFile {
                    path: path.clone(),
                    e,
                })?;
            let module_source: syn::File =
                syn::parse_file(&module_source).map_err(|e| Error::SynParseError {
                    module: module_name,
                    e,
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

mod attrs;
mod expand_impl;
