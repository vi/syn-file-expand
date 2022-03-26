use quote::{quote as q, ToTokens};

use syn_file_expand::ResolverHelper as H;

#[test]
fn trivial() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        fn lol(){}
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(|_m, _p| Ok(None), |_cfg| Ok(false)),
    )
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

    syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(
            |m: syn::Path, p| {
                if p == std::path::PathBuf::from("qqq/mod.rs") {
                    return Ok(None);
                }
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
            },
            |_cfg| Ok(false),
        ),
    )
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

    syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(
            |m: syn::Path, p: std::path::PathBuf| {
                let p = p.as_os_str().to_string_lossy();

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
            },
            |_cfg| Ok(false),
        ),
    )
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

    syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(
            |m: syn::Path, p: std::path::PathBuf| {
                let p = p.as_os_str().to_string_lossy();

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
                    "www.rs" => return Ok(None),
                    "www/mod.rs" => {
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
                    "www/../../eee.rs" => {
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
                    "www/../../rrr.rs" => Ok(None),
                    "www/../../rrr/mod.rs" => Ok(None),
                    x => panic!("surpise path: {}", x),
                }
            },
            |_cfg| Ok(false),
        ),
    )
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

#[test]
fn cfg1() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        #[cfg_attr(feature="lol",path="lol.rs")]
        #[cfg_attr(windows,path="win.rs")]
        #[cfg_attr(qqq,www)]
        mod qqq;
        fn lol(){}
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(
            |_m, p: std::path::PathBuf| {
                let p = p.as_os_str().to_string_lossy();
                match p.as_ref() {
                    "lol.rs" => Ok(None),
                    "win.rs" => Ok(Some(
                        syn::parse2(q! {
                            #![lol]
                        })
                        .unwrap(),
                    )),
                    x => panic!("Unexpected callback: {}", x),
                }
            },
            |cfg| {
                let c = cfg.into_token_stream().to_string();
                Ok(match c.as_ref() {
                    "feature = \"lol\"" => false,
                    "windows" => true,
                    x => panic!("Unexpected cfg call `{}`", x),
                })
            },
        ),
    )
    .unwrap();

    //println!("{}", before.to_token_stream().to_string());

    let after: syn::File = syn::parse2(q! {
        struct Qqq;
        #[cfg_attr(qqq,www)]
        mod qqq {
            #![lol]
        }
        fn lol(){}
    })
    .unwrap();

    assert_eq!(before, after);
}

#[test]
fn cfg2() {
    let mut before: syn::File = syn::parse2(q! {
        struct Qqq;
        #[cfg_attr(feature="lol",path="lol.rs")]
        #[cfg_attr(windows,path="win.rs")]
        mod qqq;
        fn lol(){}
    })
    .unwrap();

    let ret = syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(
            |_m, p: std::path::PathBuf| {
                let p = p.as_os_str().to_string_lossy();
                match p.as_ref() {
                    "lol.rs" | "win.rs" => Ok(Some(
                        syn::parse2(q! {
                            #![lol]
                        })
                        .unwrap(),
                    )),
                    x => panic!("Unexpected callback: {}", x),
                }
            },
            |cfg| {
                let c = cfg.into_token_stream().to_string();
                Ok(match c.as_ref() {
                    "feature = \"lol\"" => true,
                    "windows" => true,
                    x => panic!("Unexpected cfg call `{}`", x),
                })
            },
        ),
    );

    assert!(matches!(
        ret,
        Err(syn_file_expand::Error {
            inner: syn_file_expand::ErrorCase::MultipleExplicitPathsSpecifiedForOneModule,
            ..
        })
    ));
}

#[test]
fn cfg3() {
    let mut before: syn::File = syn::parse2(q! {
        #[cfg(a1)]
        struct Qqq;
        #[cfg(a2)]
        mod qqq;
        #[cfg(a3)]
        mod www;
        #[cfg(a4)]
        mod eee {
            fn lol(){}
        }
    })
    .unwrap();

    syn_file_expand::expand_modules_into_inline_modules(
        &mut before,
        &mut H(
            |_m, p: std::path::PathBuf| {
                let p = p.as_os_str().to_string_lossy();
                match p.as_ref() {
                    "qqq/mod.rs" => Ok(None),
                    "qqq.rs" => Ok(Some(
                        syn::parse2(q! {
                            #![lol]
                        })
                        .unwrap(),
                    )),
                    x => panic!("Unexpected callback: {}", x),
                }
            },
            |cfg| {
                let c = cfg.into_token_stream().to_string();
                Ok(match c.as_ref() {
                    "a2" => true,
                    "a3" => false,
                    x => panic!("Unexpected cfg call `{}`", x),
                })
            },
        ),
    )
    .unwrap();

    //println!("{}", before.to_token_stream().to_string());

    let after: syn::File = syn::parse2(q! {
        #[cfg(a1)]
        struct Qqq;
        mod qqq {
            #![lol]
        }
        #[cfg(a3)]
        mod www;
        #[cfg(a4)]
        mod eee {
            fn lol(){}
        }
    })
    .unwrap();

    assert_eq!(before, after);
}
