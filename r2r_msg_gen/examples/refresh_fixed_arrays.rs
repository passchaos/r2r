//! Refresh only fixed-array mappings in saved documentation bindings.
#[cfg(test)]
use quote::quote;
use std::{collections::HashMap, path::Path};
use syn::{
    parse_quote, visit_mut::VisitMut, Expr, Fields, GenericArgument, ImplItem, Item, Member,
    PathArguments, Type,
};

type NativeArrays = HashMap<String, HashMap<String, Expr>>;

fn native_arrays(items: &[Item]) -> NativeArrays {
    items
        .iter()
        .filter_map(|item| {
            let Item::Struct(item) = item else {
                return None;
            };
            let Fields::Named(fields) = &item.fields else {
                return None;
            };
            let arrays = fields
                .named
                .iter()
                .filter_map(|field| {
                    let Type::Array(array) = &field.ty else {
                        return None;
                    };
                    Some((field.ident.as_ref().unwrap().to_string(), array.len.clone()))
                })
                .collect();
            Some((item.ident.to_string(), arrays))
        })
        .collect()
}

fn vector_element(ty: &Type) -> Option<Type> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.first()? {
        GenericArgument::Type(ty) => Some(ty.clone()),
        _ => None,
    }
}

struct FieldsFromNative {
    name: String,
    fields: HashMap<String, Type>,
}
impl VisitMut for FieldsFromNative {
    fn visit_expr_struct_mut(&mut self, node: &mut syn::ExprStruct) {
        if node
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == self.name || segment.ident == "Self")
        {
            for value in &mut node.fields {
                let Member::Named(name) = &value.member else {
                    continue;
                };
                let Some(element) = self.fields.get(&name.to_string()) else {
                    continue;
                };
                let Type::Path(path) = element else {
                    panic!("unexpected array element type")
                };
                let leaf = path.path.segments.last().unwrap().ident.to_string();
                value.expr = if leaf == "String" {
                    parse_quote!(std::array::from_fn(|index|msg.#name[index].to_str().to_owned()))
                } else if [
                    "bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "u128", "f32",
                    "f64", "c_char",
                ]
                .contains(&leaf.as_str())
                {
                    parse_quote!(msg.#name)
                } else {
                    parse_quote!(std::array::from_fn(|index|#element::from_native(&msg.#name[index])))
                };
            }
        }
        syn::visit_mut::visit_expr_struct_mut(self, node);
    }
}

fn refresh(items: &mut [Item], native: &NativeArrays) -> usize {
    let mut count = 0;
    for item in items.iter_mut() {
        if let Item::Mod(module) = item {
            if let Some((_, items)) = &mut module.content {
                count += refresh(items, native);
            }
        }
    }
    let mappings: Vec<_> = items
        .iter()
        .filter_map(|item| {
            let Item::Impl(item) = item else { return None };
            let Type::Path(path) = &*item.self_ty else {
                return None;
            };
            let name = path.path.segments.last()?.ident.to_string();
            let ty = item.items.iter().find_map(|item| match item {
                ImplItem::Type(item) if item.ident == "CStruct" => Some(&item.ty),
                _ => None,
            })?;
            let Type::Path(path) = ty else { return None };
            Some((
                name,
                native
                    .get(&path.path.segments.last()?.ident.to_string())?
                    .clone(),
            ))
        })
        .collect();
    for (name, arrays) in mappings {
        let mut fields = HashMap::new();
        for item in items.iter_mut() {
            let Item::Struct(item) = item else { continue };
            if item.ident != name {
                continue;
            }
            let Fields::Named(named) = &mut item.fields else {
                continue;
            };
            for field in &mut named.named {
                let key = field.ident.as_ref().unwrap().to_string();
                let Some(length) = arrays.get(&key) else {
                    continue;
                };
                let Some(element) = vector_element(&field.ty) else {
                    continue;
                };
                fields.insert(key, element.clone());
                field.ty = parse_quote!([#element;#length]);
                field
                    .attrs
                    .push(parse_quote!(#[serde(with="crate::msg_types::fixed_array")]));
                count += 1;
            }
        }
        if fields.is_empty() {
            continue;
        }
        for item in items.iter_mut() {
            let Item::Impl(item) = item else { continue };
            let Type::Path(path) = &*item.self_ty else {
                continue;
            };
            if path.path.segments.last().unwrap().ident != name {
                continue;
            }
            for item in &mut item.items {
                if let ImplItem::Fn(function) = item {
                    if function.sig.ident == "from_native" {
                        FieldsFromNative {
                            name: name.clone(),
                            fields: fields.clone(),
                        }
                        .visit_block_mut(&mut function.block);
                    }
                }
            }
        }
    }
    count
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 3 || arguments[2] != "--write" {
        return Err(
            "usage: refresh_fixed_arrays <native-bindings.rs> <saved-directory> --write".into()
        );
    }
    let native = syn::parse_file(&std::fs::read_to_string(&arguments[0])?)?;
    let arrays = native_arrays(&native.items);
    let mut total = 0;
    for entry in std::fs::read_dir(Path::new(&arguments[1]))? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "rs")
            || path.file_name().unwrap().to_string_lossy().starts_with('_')
        {
            continue;
        }
        let mut file = syn::parse_file(&std::fs::read_to_string(&path)?)?;
        let count = refresh(&mut file.items, &arrays);
        if count > 0 {
            std::fs::write(&path, prettyplease::unparse(&file))?;
        }
        total += count;
    }
    println!("refreshed {total} fixed-array fields in saved documentation bindings");
    Ok(())
}

#[test]
fn fixed_fields_change_while_sequences_keep_their_representation() {
    let native = syn::parse_file("struct Native { fixed:[u8;16], sequence:Sequence }").unwrap();
    let mut rust=syn::parse_file("struct Msg { fixed:Vec<u8>, sequence:Vec<u8> } impl WrappedTypesupport for Msg { type CStruct=Native; fn from_native(msg:&Native)->Msg { Msg {fixed:msg.fixed.to_vec(), sequence:msg.sequence.to_vec()} } }").unwrap();
    assert_eq!(refresh(&mut rust.items, &native_arrays(&native.items)), 1);
    let text = quote!(#rust).to_string();
    assert!(text.contains("fixed : [u8 ; 16]"));
    assert!(text.contains("sequence : Vec < u8 >"));
    assert!(text.contains("fixed : msg . fixed"));
    assert_eq!(refresh(&mut rust.items, &native_arrays(&native.items)), 0);
}
