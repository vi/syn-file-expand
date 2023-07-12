use proc_macro2::Span;

fn loopify_block(block: &mut syn::Block) {
    block.stmts = Vec::with_capacity(1);
    block
        .stmts
        .push(syn::Stmt::Expr(syn::Expr::Loop(syn::ExprLoop {
            attrs: vec![],
            label: None,
            loop_token: syn::Token!(loop)(Span::call_site()),
            body: syn::Block {
                brace_token: syn::token::Brace::default(),
                stmts: vec![],
            },
        }), None));
}

fn loopify_expr(expr: &mut syn::Expr) {
    *expr = syn::Expr::Loop(syn::ExprLoop {
        attrs: vec![],
        label: None,
        loop_token: syn::Token!(loop)(Span::call_site()),
        body: syn::Block {
            brace_token: syn::token::Brace::default(),
            stmts: vec![],
        },
    });
}

fn loopify_macro(_m: &mut syn::Macro) {
    // skipped for now
}

fn loopify_item(item: &mut syn::Item) {
    match item {
        syn::Item::Const(x) => loopify_expr(&mut x.expr),
        syn::Item::Trait(x) => {
            for item in &mut x.items {
                match item {
                    syn::TraitItem::Const(y) => {
                        if let Some(z) = &mut y.default {
                            loopify_expr(&mut z.1);
                        }
                    }
                    syn::TraitItem::Fn(y) => {
                        if let Some(z) = &mut y.default {
                            loopify_block(z);
                        }
                    }
                    syn::TraitItem::Type(_) => (),
                    syn::TraitItem::Macro(y) => loopify_macro(&mut y.mac),
                    syn::TraitItem::Verbatim(_) => (),
                    _ => (),
                }
            }
        }
        syn::Item::Static(x) => loopify_expr(&mut x.expr),
        syn::Item::Macro(x) => loopify_macro(&mut x.mac),
        syn::Item::Mod(x) => {
            if let Some(ref mut content) = x.content {
                for subitem in &mut content.1 {
                    loopify_item(subitem);
                }
            }
        }
        syn::Item::Impl(x) => {
            for subitem in &mut x.items {
                match subitem {
                    syn::ImplItem::Const(y) => {
                        loopify_expr(&mut y.expr);
                    }
                    syn::ImplItem::Fn(y) => {
                        loopify_block(&mut y.block);
                    }
                    syn::ImplItem::Macro(y) => {
                        loopify_macro(&mut y.mac);
                    }
                    syn::ImplItem::Type(_) => (),
                    syn::ImplItem::Verbatim(_) => (),
                    _ => (),
                }
            }
        }
        syn::Item::Fn(x) => loopify_block(&mut x.block),
        syn::Item::ForeignMod(_) => (),
        syn::Item::ExternCrate(_) => (),
        syn::Item::Enum(_) => (),
        syn::Item::Struct(_) => (),
        syn::Item::TraitAlias(_) => (),
        syn::Item::Type(_) => (),
        syn::Item::Union(_) => (),
        syn::Item::Verbatim(_) => (),
        syn::Item::Use(_) => (),
        _ => (),
    }
}

pub fn loopify(f: &mut syn::File) {
    for item in &mut f.items {
        loopify_item(item);
    }
}
