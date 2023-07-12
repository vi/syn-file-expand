use quote::quote as q;
//use pretty_assertions::assert_eq;

#[test]
fn fullsource_plain() {
    let mut sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sample.push("resources"); 
    sample.push("sample"); 
    sample.push("lib.rs"); 

    let src = syn_file_expand::read_full_crate_source_code(sample, |_|Ok(false)).unwrap();

    let expected : syn::File = syn::parse2(q!{
        mod plain {
            mod plain_inner {}
            mod plain_inner_path {}
        }
        mod with_mod {
            mod with_mod_inner {}
            mod with_mod_inner_path {}
        }
        mod plain_path {
            mod plain_path_inner {}
            mod plain_path_inner_path {}
        }
        mod with_mod_path {
            mod with_mod_path_inner {}
            mod with_mod_path_inner_path {}
        }
    }).unwrap();

    assert_eq!(prettyplease::unparse(&src), prettyplease::unparse(&expected));
}

#[test]
fn fullsource_withdup() {
    let mut sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sample.push("resources"); 
    sample.push("withdup"); 
    sample.push("lib.rs"); 

    let src = syn_file_expand::read_crate(sample).unwrap();

    let expected : syn::File = syn::parse2(q!{
        mod duplicate_plain {
            struct DuplicatePlainMod;
        }

        mod with_path {
            struct A;
        }

        #[cfg(feature="b")]
        mod tricky {
            struct B;
        }

        #[cfg(feature="c")]
        mod tricky {
            struct C;
        }

        #[cfg(not(any(feature="b", feature="c")))]
        mod tricky {
            struct Tricky;
        }

    }).unwrap();

    assert_eq!(prettyplease::unparse(&src), prettyplease::unparse(&expected));
}
