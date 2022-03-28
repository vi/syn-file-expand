
fn undoc_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| {
        match attr.path.get_ident().map(|x|x.to_string()).as_deref() {
            Some("doc") => false,
            _ => true,
        }
    });
}
fn undoc_item(item: &mut syn::Item) {
    match item {
        syn::Item::Const(x) => undoc_attrs(&mut x.attrs),
        syn::Item::Trait(x) => {
            undoc_attrs(&mut x.attrs);
            for item in &mut x.items {
                match item {
                    syn::TraitItem::Const(y) => {
                        undoc_attrs(&mut y.attrs)
                    }
                    syn::TraitItem::Method(y) => {
                        undoc_attrs(&mut y.attrs)
                    }
                    syn::TraitItem::Type(y) => undoc_attrs(&mut y.attrs),
                    syn::TraitItem::Macro(y) => undoc_attrs(&mut y.attrs),
                    syn::TraitItem::Verbatim(_) => (),
                    _ => (),
                }
            }
        }
        syn::Item::Static(x) => undoc_attrs(&mut x.attrs),
        syn::Item::Macro(x)  => undoc_attrs(&mut x.attrs),
        syn::Item::Mod(x) => {
            undoc_attrs(&mut x.attrs);
            if let Some(ref mut content) = x.content {
                for subitem in &mut content.1 {
                    undoc_item(subitem);
                }
            }
        }
        syn::Item::Impl(x) => {
            undoc_attrs(&mut x.attrs);
            for subitem in &mut x.items {
                match subitem {
                    syn::ImplItem::Const(y) => undoc_attrs(&mut y.attrs),
                    syn::ImplItem::Method(y) =>undoc_attrs(&mut y.attrs),
                    syn::ImplItem::Macro(y)  =>undoc_attrs(&mut y.attrs),
                    syn::ImplItem::Type(y)  =>undoc_attrs(&mut y.attrs),
                    syn::ImplItem::Verbatim(_) => (),
                    _ => (),
                }
            }
        }
        syn::Item::Fn(x) =>undoc_attrs(&mut x.attrs),
        syn::Item::ForeignMod(x)=>undoc_attrs(&mut x.attrs),
        syn::Item::ExternCrate(x)=>undoc_attrs(&mut x.attrs),
        syn::Item::Enum(x)=>{
            undoc_attrs(&mut x.attrs);
            for y in &mut x.variants {
                undoc_attrs(&mut y.attrs);
            }
        }
        syn::Item::Struct(x)=>{
            undoc_attrs(&mut x.attrs);
            for y in &mut x.fields {
                undoc_attrs(&mut y.attrs);
            }
        }
        syn::Item::TraitAlias(x)=>undoc_attrs(&mut x.attrs),
        syn::Item::Type(x) =>undoc_attrs(&mut x.attrs),
        syn::Item::Union(x)=> {
            undoc_attrs(&mut x.attrs);
            for y in &mut x.fields.named {
                undoc_attrs(&mut y.attrs);
            }
        }
        syn::Item::Verbatim(_) => (),
        syn::Item::Use(_) => (),
        _ => (),
    }
}

pub fn undoc(f: &mut syn::File) {
    undoc_attrs(&mut f.attrs);
    for item in &mut f.items {
        undoc_item(item);
    }
}
