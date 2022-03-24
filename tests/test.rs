use quote::quote as q;

#[test]
fn trivial() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        fn lol(){}
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(&mut before, &mut |_m, _p, _c| Ok(None))
        .unwrap();

    let after: syn::File = syn::parse2(q! {
        struct Qqq;
        fn lol(){}
    })
    .unwrap();

    assert_eq!(before, after);
}

#[test]
fn simple() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        mod qqq;
        fn lol(){}
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(&mut before, &mut |m: syn::Path,
                                                                           p,
                                                                           c: Option<
        syn::Meta,
    >| {
        if p == std::path::PathBuf::from("qqq/mod.rs") {
            return Ok(None);
        }
        assert!(c.is_none());
        assert_eq!(
            m.segments
                .into_iter()
                .map(|x| x.ident.to_string())
                .collect::<Vec<_>>(),
            vec!["qqq".to_owned()]
        );
        assert_eq!(p, std::path::PathBuf::from("qqq.rs"));
        Ok(Some(
            syn::parse2(q! {
                trait Ror {
                }
            })
            .unwrap(),
        ))
    })
    .unwrap();

    let after: syn::File = syn::parse2(q! {
        struct Qqq;
        mod qqq {
            trait Ror {
            }
        }
        fn lol(){}
    })
    .unwrap();

    assert_eq!(before, after);
}

#[test]
fn nested1() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        mod qqq;
        fn lol(){}
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(&mut before, &mut |m: syn::Path,
                                                                           p: std::path::PathBuf,
                                                                           c: Option<
        syn::Meta,
    >| {
        let p = p.as_os_str().to_string_lossy();

        assert!(c.is_none());
        match p.as_ref() {
            "qqq/mod.rs" => return Ok(None),
            "qqq.rs" => {
                assert_eq!(
                    m.segments
                        .into_iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<_>>(),
                    vec!["qqq".to_owned()]
                );
                Ok(Some(
                    syn::parse2(q! {
                        trait Ror {
                        }
                        mod www;
                    })
                    .unwrap(),
                ))
            }
            "qqq/www.rs" => return Ok(None),
            "qqq/www/mod.rs" => {
                assert_eq!(
                    m.segments
                        .into_iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<_>>(),
                    vec!["qqq".to_owned(), "www".to_owned()]
                );
                Ok(Some(
                    syn::parse2(q! {
                        mod eee;
                        type Q = i32;
                    })
                    .unwrap(),
                ))
            }
            "qqq/www/eee.rs" => {
                assert_eq!(
                    m.segments
                        .into_iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<_>>(),
                    vec!["qqq".to_owned(), "www".to_owned(), "eee".to_owned()]
                );
                Ok(Some(
                    syn::parse2(q! {
                        fn r(_x:i32){}
                    })
                    .unwrap(),
                ))
            }
            "qqq/www/eee/mod.rs" => return Ok(None),
            x => panic!("surpise path: {}", x),
        }
    })
    .unwrap();

    let after: syn::File = syn::parse2(q! {
        struct Qqq;
        mod qqq {
            trait Ror {
            }
            mod www {
                mod eee {
                    fn r(_x:i32){}
                }
                type Q = i32;
            }
        }
        fn lol(){}
    })
    .unwrap();

    assert_eq!(before, after);
}


#[test]
fn explicit_paths() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        #[path="qqq.rs"]
        mod qqq;
        fn lol(){}
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(&mut before, &mut |m: syn::Path,
                                                                           p: std::path::PathBuf,
                                                                           c: Option<
        syn::Meta,
    >| {
        let p = p.as_os_str().to_string_lossy();

        assert!(c.is_none());
        match p.as_ref() {
            "qqq.rs" => {
                assert_eq!(
                    m.segments
                        .into_iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<_>>(),
                    vec!["qqq".to_owned()]
                );
                Ok(Some(
                    syn::parse2(q! {
                        trait Ror {
                        }
                        mod www;
                    })
                    .unwrap(),
                ))
            }
            "qqq/www.rs" => return Ok(None),
            "qqq/www/mod.rs" => {
                assert_eq!(
                    m.segments
                        .into_iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<_>>(),
                    vec!["qqq".to_owned(), "www".to_owned()]
                );
                Ok(Some(
                    syn::parse2(q! {
                        #[path="../../eee.rs"]
                        mod eee;
                        type Q = i32;
                    })
                    .unwrap(),
                ))
            }
            "qqq/www/../../eee.rs" => {
                assert_eq!(
                    m.segments
                        .into_iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<_>>(),
                    vec!["qqq".to_owned(), "www".to_owned(), "eee".to_owned()]
                );
                Ok(Some(
                    syn::parse2(q! {
                        fn r(_x:i32){}
                        pub mod rrr;
                    })
                    .unwrap(),
                ))
            }
            "qqq/www/../../eee/rrr.rs" => Ok(None),
            "qqq/www/../../eee/rrr/mod.rs" => Ok(None),
            x => panic!("surpise path: {}", x),
        }
    })
    .unwrap();

    let after: syn::File = syn::parse2(q! {
        struct Qqq;
        mod qqq {
            trait Ror {
            }
            mod www {
                mod eee {
                    fn r(_x:i32){}
                    pub mod rrr;
                }
                type Q = i32;
            }
        }
        fn lol(){}
    })
    .unwrap();

    assert_eq!(before, after);
}
