//! Refresh saved ROS string fields from native C metadata without adding distribution APIs.
use quote::quote;
use std::{collections::HashMap, path::Path};
use syn::{parse_quote, visit_mut::VisitMut, Expr, Fields, ImplItem, Item, Member, Type};

#[derive(Clone, Copy)]
enum Shape {
    Scalar,
    Sequence,
    Array,
}
#[derive(Clone, Copy)]
struct StringField {
    wide: bool,
    shape: Shape,
}
type FieldsMap = HashMap<String, HashMap<String, StringField>>;

fn string_field(ty: &Type) -> Option<StringField> {
    if let Type::Array(array) = ty {
        return string_field(&array.elem).map(|field| StringField {
            shape: Shape::Array,
            ..field
        });
    }
    let Type::Path(path) = ty else { return None };
    let name = path.path.segments.last()?.ident.to_string();
    let (wide, shape) = match name.as_str() {
        "rosidl_runtime_c__String" => (false, Shape::Scalar),
        "rosidl_runtime_c__U16String" => (true, Shape::Scalar),
        "rosidl_runtime_c__String__Sequence" => (false, Shape::Sequence),
        "rosidl_runtime_c__U16String__Sequence" => (true, Shape::Sequence),
        _ => return None,
    };
    Some(StringField { wide, shape })
}
fn field_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Field(field) => match &field.member {
            Member::Named(name) => Some(name.to_string()),
            _ => None,
        },
        Expr::MethodCall(call) => field_name(&call.receiver),
        Expr::Paren(expr) => field_name(&expr.expr),
        _ => None,
    }
}
struct Conversion<'a> {
    name: &'a str,
    fields: &'a HashMap<String, StringField>,
    loop_wide: Option<bool>,
}
impl VisitMut for Conversion<'_> {
    fn visit_expr_struct_mut(&mut self, node: &mut syn::ExprStruct) {
        if node
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == self.name || segment.ident == "Self")
        {
            for field in &mut node.fields {
                let Member::Named(name) = &field.member else {
                    continue;
                };
                let Some(kind) = self.fields.get(&name.to_string()) else {
                    continue;
                };
                let (wrapper, read) = if kind.wide {
                    (quote!(crate::RosWString), quote!(as_units))
                } else {
                    (quote!(crate::RosString), quote!(as_bytes))
                };
                field.expr = match kind.shape {
                    Shape::Scalar => parse_quote!(#wrapper::from(msg.#name.#read())),
                    Shape::Array => {
                        parse_quote!(std::array::from_fn(|index|#wrapper::from(msg.#name[index].#read())))
                    }
                    Shape::Sequence => {
                        if kind.wide {
                            parse_quote!(msg.#name.to_units_vec::<crate::RosWString>())
                        } else {
                            parse_quote!(msg.#name.to_bytes_vec::<crate::RosString>())
                        }
                    }
                };
            }
        }
        syn::visit_mut::visit_expr_struct_mut(self, node);
    }
    fn visit_expr_for_loop_mut(&mut self, node: &mut syn::ExprForLoop) {
        let previous = self.loop_wide;
        self.loop_wide = field_name(&node.expr)
            .and_then(|name| self.fields.get(&name))
            .and_then(|kind| match kind.shape {
                Shape::Array => Some(kind.wide),
                _ => None,
            });
        self.visit_block_mut(&mut node.body);
        self.loop_wide = previous;
    }
    fn visit_expr_method_call_mut(&mut self, node: &mut syn::ExprMethodCall) {
        let kind = field_name(&node.receiver)
            .and_then(|name| self.fields.get(&name))
            .copied();
        if node.method == "assign" || node.method == "try_assign" {
            let wide = kind.map(|kind| kind.wide).or(self.loop_wide);
            if let Some(wide) = wide {
                let argument = node.args.first().cloned().unwrap();
                node.method = if wide {
                    parse_quote!(try_assign_units)
                } else {
                    parse_quote!(try_assign_bytes)
                };
                node.args[0] = if wide {
                    parse_quote!((#argument).as_units())
                } else {
                    parse_quote!((#argument).as_bytes())
                };
            }
        } else if node.method == "update" || node.method == "try_update" {
            if let Some(kind) = kind {
                node.method = if kind.wide {
                    parse_quote!(try_update_units)
                } else {
                    parse_quote!(try_update_bytes)
                };
            }
        }
        syn::visit_mut::visit_expr_method_call_mut(self, node);
    }
}

fn refresh(items: &mut [Item], native: &FieldsMap) -> usize {
    let mut total = 0;
    for item in items.iter_mut() {
        if let Item::Mod(module) = item {
            if let Some((_, items)) = &mut module.content {
                total += refresh(items, native);
            }
        }
    }
    let maps: Vec<_> = items
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
    for (name, fields) in maps {
        for item in items.iter_mut() {
            let Item::Struct(item) = item else { continue };
            if item.ident != name {
                continue;
            }
            let Fields::Named(named) = &mut item.fields else {
                continue;
            };
            for field in &mut named.named {
                let Some(kind) = fields.get(&field.ident.as_ref().unwrap().to_string()) else {
                    continue;
                };
                let element: Type = if kind.wide {
                    parse_quote!(crate::RosWString)
                } else {
                    parse_quote!(crate::RosString)
                };
                field.ty = match kind.shape {
                    Shape::Scalar => element,
                    Shape::Sequence => parse_quote!(Vec<#element>),
                    Shape::Array => {
                        let Type::Array(array) = &field.ty else {
                            panic!("refresh fixed arrays before strings")
                        };
                        let length = &array.len;
                        parse_quote!([#element;#length])
                    }
                };
                total += 1;
            }
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
                    if ["from_native", "copy_to_native", "try_copy_to_native"]
                        .iter()
                        .any(|&name| function.sig.ident == name)
                    {
                        Conversion {
                            name: &name,
                            fields: &fields,
                            loop_wide: None,
                        }
                        .visit_block_mut(&mut function.block);
                    }
                }
            }
        }
    }
    total
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 3 || arguments[2] != "--write" {
        return Err("usage: refresh_strings <native-bindings.rs> <saved-directory> --write".into());
    }
    let file = syn::parse_file(&std::fs::read_to_string(&arguments[0])?)?;
    let native: FieldsMap = file
        .items
        .iter()
        .filter_map(|item| {
            let Item::Struct(item) = item else {
                return None;
            };
            let Fields::Named(fields) = &item.fields else {
                return None;
            };
            Some((
                item.ident.to_string(),
                fields
                    .named
                    .iter()
                    .filter_map(|field| {
                        string_field(&field.ty)
                            .map(|kind| (field.ident.as_ref().unwrap().to_string(), kind))
                    })
                    .collect(),
            ))
        })
        .collect();
    let mut total = 0;
    for entry in std::fs::read_dir(Path::new(&arguments[1]))? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "rs")
            || path.file_name().unwrap().to_string_lossy().starts_with('_')
        {
            continue;
        }
        let mut file = syn::parse_file(&std::fs::read_to_string(&path)?)?;
        let count = refresh(&mut file.items, &native);
        if count > 0 {
            std::fs::write(&path, prettyplease::unparse(&file))?;
        }
        total += count;
    }
    println!("refreshed {total} saved raw string fields");
    Ok(())
}

#[test]
fn scalar_sequence_and_fixed_string_fields_keep_raw_payloads() {
    let fields = HashMap::from([
        (
            "text".into(),
            StringField {
                wide: false,
                shape: Shape::Scalar,
            },
        ),
        (
            "wide".into(),
            StringField {
                wide: true,
                shape: Shape::Sequence,
            },
        ),
        (
            "fixed".into(),
            StringField {
                wide: false,
                shape: Shape::Array,
            },
        ),
    ]);
    let mut file=syn::parse_file("struct Msg { text:String,wide:Vec<String>,fixed:[String;3] } impl WrappedTypesupport for Msg { type CStruct=Native; fn from_native(msg:&Native)->Msg { Msg { text:msg.text.to_str().to_owned(),wide:msg.wide.to_vec(),fixed:std::array::from_fn(|i|msg.fixed[i].to_str().to_owned())} } fn copy_to_native(&self,msg:&mut Native) { msg.text.assign(&self.text);msg.wide.update(&self.wide);for (t,s) in msg.fixed.iter_mut().zip(&self.fixed) {t.assign(s);} } }").unwrap();
    assert_eq!(refresh(&mut file.items, &HashMap::from([("Native".into(), fields)])), 3);
    let text = quote!(#file).to_string();
    assert!(text.contains("crate :: RosString"));
    assert!(text.contains("to_units_vec"));
    assert!(text.contains("try_assign_bytes"));
    assert!(text.contains("try_update_units"));
}
