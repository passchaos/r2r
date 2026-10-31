//! Replace interface FFI imports with cached, ABI-matching lazy wrappers.
use quote::{format_ident, quote};
use syn::{FnArg, Item, Pat, ReturnType, Type};

fn interface(name: &str) -> Option<(&str, &str)> {
    for (prefix, kind) in [
        ("rosidl_typesupport_introspection_c__", "Introspection"),
        ("rosidl_typesupport_c__", "Typesupport"),
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            let package = rest.split("__").nth(1)?;
            return Some((package, kind));
        }
    }
    let (package, rest) = name.split_once("__")?;
    if ["msg__", "srv__", "action__"]
        .iter()
        .any(|prefix| rest.starts_with(prefix))
    {
        Some((package, "Generator"))
    } else {
        None
    }
}

pub fn transform(mut file: syn::File) -> syn::File {
    let mut items = Vec::new();
    for item in file.items {
        let Item::ForeignMod(mut foreign) = item else {
            items.push(item);
            continue;
        };
        let mut remaining = Vec::new();
        for item in foreign.items {
            let syn::ForeignItem::Fn(function) = item else {
                remaining.push(item);
                continue;
            };
            let name = function.sig.ident.to_string();
            let Some((package, kind)) = interface(&name) else {
                remaining.push(syn::ForeignItem::Fn(function));
                continue;
            };
            assert!(
                function.sig.variadic.is_none(),
                "variadic interface functions are unsupported"
            );
            let kind = format_ident!("{kind}");
            let helper = format_ident!("r2r_resolve_{name}");
            let mut signature = function.sig.clone();
            signature.unsafety = Some(syn::token::Unsafe::default());
            signature.abi = Some(syn::parse_quote!(extern "C"));
            let mut types = Vec::new();
            let mut arguments = Vec::new();
            for input in &signature.inputs {
                let FnArg::Typed(argument) = input else {
                    panic!("interface receiver");
                };
                let Pat::Ident(identifier) = argument.pat.as_ref() else {
                    panic!("interface argument must be named");
                };
                types.push(argument.ty.as_ref());
                arguments.push(&identifier.ident);
            }
            let output = &signature.output;
            let fallback = match output {
                ReturnType::Default => quote!(()),
                ReturnType::Type(_, ty) => match ty.as_ref() {
                    Type::Ptr(ptr) if ptr.mutability.is_some() => quote!(std::ptr::null_mut()),
                    Type::Ptr(_) => quote!(std::ptr::null()),
                    Type::Path(path) if path.path.is_ident("bool") => quote!(false),
                    Type::Tuple(tuple) if tuple.elems.is_empty() => quote!(()),
                    _ => panic!("unsupported interface result for {name}"),
                },
            };
            let bytes =
                syn::LitByteStr::new(format!("{name}\0").as_bytes(), function.sig.ident.span());
            let attrs = &function.attrs;
            let visibility = &function.vis;
            items.push(syn::parse2(quote! {
                #[doc(hidden)]
                #[inline]
                pub fn #helper()->Result<unsafe extern "C" fn(#(#types),*) #output,crate::native_library::LoadError> {
                    type Operation=unsafe extern "C" fn(#(#types),*) #output;
                    static OPERATION:std::sync::OnceLock<Operation>=std::sync::OnceLock::new();
                    if let Some(operation)=OPERATION.get(){return Ok(*operation);}
                    let name=unsafe {std::ffi::CStr::from_bytes_with_nul_unchecked(#bytes)};
                    let operation=unsafe {crate::native_library::symbol::<Operation>(#package,crate::native_library::Kind::#kind,name)}?;
                    Ok(*OPERATION.get_or_init(||operation))
                }
            }).unwrap());
            items.push(
                syn::parse2(quote! {
                    #(#attrs)*
                    #visibility #signature {
                        match #helper() {
                            Ok(operation)=>unsafe{operation(#(#arguments),*)},
                            Err(_)=>#fallback,
                        }
                    }
                })
                .unwrap(),
            );
        }
        foreign.items = remaining;
        if !foreign.items.is_empty() {
            items.push(Item::ForeignMod(foreign));
        }
    }
    file.items = items;
    file
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lazy_interface_functions_classify_library_without_foreign_relocations() {
        let input=syn::parse_file("extern \"C\" {pub fn rosidl_typesupport_c__get_service_type_support_handle__example_interfaces__srv__AddTwoInts()->*const u8;pub fn example_interfaces__srv__AddTwoInts_Request__create()->*mut u8;pub fn example_interfaces__srv__AddTwoInts_Request__destroy(ptr:*mut u8);pub fn example_interfaces__srv__AddTwoInts_Request__Sequence__init(ptr:*mut u8,len:usize)->bool;pub fn ordinary_external()->u32;}").unwrap();
        let output = transform(input);
        let text = quote!(#output).to_string();
        assert!(text.contains("Kind :: Typesupport"));
        assert!(text.contains("Kind :: Generator"));
        assert!(text.contains("OnceLock"));
        assert!(text.contains("false"));
        let remaining: Vec<_> = output
            .items
            .iter()
            .filter_map(|item| {
                if let Item::ForeignMod(f) = item {
                    Some(f)
                } else {
                    None
                }
            })
            .flat_map(|f| &f.items)
            .collect();
        assert_eq!(remaining.len(), 1);
        assert!(matches!(remaining[0],syn::ForeignItem::Fn(f) if f.sig.ident=="ordinary_external"));
    }
    #[test]
    fn introspection_and_service_event_symbols_select_correct_package() {
        assert_eq!(interface("rosidl_typesupport_introspection_c__get_message_type_support_handle__std_msgs__msg__String"),Some(("std_msgs","Introspection")));
        assert_eq!(interface("rosidl_typesupport_c__create_service_event_message__example_interfaces__srv__AddTwoInts"),Some(("example_interfaces","Typesupport")));
        assert_eq!(interface("unrelated_function"), None);
    }
}
