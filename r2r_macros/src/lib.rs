use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Expr, Fields, FieldsNamed, LitBool,
    LitStr,
};

extern crate proc_macro;

/// Derives RosParams trait for a structure to use it with
/// `r2r::Node::make_derived_parameter_handler()`.
///
/// Leaf fields may use `#[ros_param(read_only)]`, `additional_constraints`,
/// `integer_range(from = ..., to = ..., step = ...)` or
/// `floating_point_range(from = ..., to = ..., step = ...)`.
#[proc_macro_derive(RosParams, attributes(ros_param))]
pub fn derive_r2r_params(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);
    expand_r2r_params(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_r2r_params(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "RosParams can only be derived for structs with named fields",
                ));
            }
        },
        Data::Enum(_) | Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "RosParams can only be derived for structs with named fields",
            ));
        }
    };

    // Used in the quasi-quotation below as `#name`.
    let name = &input.ident;

    let register_calls = get_register_calls(fields)?;
    let descriptor_register_calls = get_descriptor_register_calls(fields)?;
    let get_param_matches = param_matches_for(quote!(get_parameter(suffix)), fields);
    let set_param_matches = param_matches_for(quote!(set_parameter(suffix, param_val)), fields);
    let check_param_matches = param_matches_for(quote!(check_parameter(suffix, param_val)), fields);

    let expanded = quote! {
        // The generated impl.
        impl ::r2r::RosParams for #name {
            fn register_parameters(
                &mut self,
                prefix: &str,
                desc: ::std::option::Option<::r2r::Parameter>,
                params: &mut ::r2r::indexmap::IndexMap<String, ::r2r::Parameter>,
            ) -> ::r2r::Result<()> {
                let prefix = if prefix.is_empty() {
                    String::from("")
                } else {
                    format!("{prefix}.")
                };
                #register_calls
                Ok(())
            }
            fn register_parameter_descriptors(
                &self,
                prefix: &str,
                _descriptor: ::std::option::Option<::r2r::ParameterDescriptor>,
                descriptors: &mut ::r2r::indexmap::IndexMap<String, ::r2r::ParameterDescriptor>,
            ) -> ::r2r::Result<()> {
                let prefix = if prefix.is_empty() {
                    String::from("")
                } else {
                    format!("{prefix}.")
                };
                #descriptor_register_calls
                Ok(())
            }
            fn get_parameter(&mut self, param_name: &str) -> ::r2r::Result<::r2r::ParameterValue>
            {
                let (prefix, suffix) = match param_name.split_once('.') {
                    None => (param_name, ""),
                    Some((prefix, suffix)) => (prefix, suffix)
                };
                let result = match prefix {
                    #get_param_matches
                    _ => Err(::r2r::Error::InvalidParameterName {
                        name: "".into(),
                    }),
                };
                result.map_err(|e| e.update_param_name(&param_name))
            }
            fn set_parameter(&mut self, param_name: &str, param_val: &::r2r::ParameterValue) -> ::r2r::Result<()>
            {
                let (prefix, suffix) = match param_name.split_once('.') {
                    None => (param_name, ""),
                    Some((prefix, suffix)) => (prefix, suffix)
                };
                let result = match prefix {
                    #set_param_matches
                    _ => Err(::r2r::Error::InvalidParameterName {
                        name: "".into(),
                    }),
                };
                result.map_err(|e| e.update_param_name(&param_name))
            }
            fn check_parameter(&self, param_name: &str, param_val: &::r2r::ParameterValue) -> ::r2r::Result<()>
            {
                let (prefix, suffix) = match param_name.split_once('.') {
                    None => (param_name, ""),
                    Some((prefix, suffix)) => (prefix, suffix)
                };
                let result = match prefix {
                    #check_param_matches
                    _ => Err(::r2r::Error::InvalidParameterName {
                        name: "".into(),
                    }),
                };
                result.map_err(|e| e.update_param_name(&param_name))
            }
        }
    };

    // Hand the output tokens back to the compiler.
    Ok(expanded)
}

// Generate calls to register functions of struct fields
fn get_register_calls(fields: &FieldsNamed) -> syn::Result<TokenStream> {
    let field_matches = fields
        .named
        .iter()
        .map(|f| {
            let name = &f.ident;
            let format_str = format!("{{prefix}}{}", name.as_ref().expect("named field"));
            let desc = get_field_doc(f)?;
            Ok(quote_spanned! {
                f.span() =>
                    let param = ::r2r::Parameter {
                        value: ::r2r::ParameterValue::NotSet, // will be set for leaf params by register_parameters() below
                        description: #desc,
                    };
                    self.#name.register_parameters(&format!(#format_str), Some(param), params)?;
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        #(#field_matches)*
    })
}

struct RangeOptions {
    from: Expr,
    to: Expr,
    step: Expr,
}

#[derive(Default)]
struct ParameterOptions {
    read_only: Option<bool>,
    additional_constraints: Option<LitStr>,
    integer_range: Option<RangeOptions>,
    floating_point_range: Option<RangeOptions>,
}

fn parse_range(meta: syn::meta::ParseNestedMeta<'_>) -> syn::Result<RangeOptions> {
    let mut from = None;
    let mut to = None;
    let mut step = None;
    meta.parse_nested_meta(|entry| {
        let target = if entry.path.is_ident("from") {
            &mut from
        } else if entry.path.is_ident("to") {
            &mut to
        } else if entry.path.is_ident("step") {
            &mut step
        } else {
            return Err(entry.error("expected `from`, `to`, or `step`"));
        };
        if target.is_some() {
            return Err(entry.error("duplicate range value"));
        }
        *target = Some(entry.value()?.parse()?);
        Ok(())
    })?;
    Ok(RangeOptions {
        from: from.ok_or_else(|| meta.error("range requires `from`"))?,
        to: to.ok_or_else(|| meta.error("range requires `to`"))?,
        step: step.ok_or_else(|| meta.error("range requires `step`"))?,
    })
}

fn get_parameter_options(field: &syn::Field) -> syn::Result<ParameterOptions> {
    let mut options = ParameterOptions::default();
    for attribute in field
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("ros_param"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("read_only") {
                if options.read_only.is_some() {
                    return Err(meta.error("duplicate `read_only` option"));
                }
                options.read_only = Some(if meta.input.peek(syn::Token![=]) {
                    meta.value()?.parse::<LitBool>()?.value
                } else {
                    true
                });
                return Ok(());
            }
            if meta.path.is_ident("additional_constraints") {
                if options.additional_constraints.is_some() {
                    return Err(meta.error("duplicate `additional_constraints` option"));
                }
                options.additional_constraints = Some(meta.value()?.parse()?);
                return Ok(());
            }
            if meta.path.is_ident("integer_range") {
                if options.integer_range.is_some() {
                    return Err(meta.error("duplicate `integer_range` option"));
                }
                options.integer_range = Some(parse_range(meta)?);
                return Ok(());
            }
            if meta.path.is_ident("floating_point_range") {
                if options.floating_point_range.is_some() {
                    return Err(meta.error("duplicate `floating_point_range` option"));
                }
                options.floating_point_range = Some(parse_range(meta)?);
                return Ok(());
            }
            Err(meta.error("unsupported `ros_param` option"))
        })?;
    }
    if options.integer_range.is_some() && options.floating_point_range.is_some() {
        return Err(syn::Error::new_spanned(
            field,
            "integer and floating-point ranges are mutually exclusive",
        ));
    }
    Ok(options)
}

fn get_descriptor_register_calls(fields: &FieldsNamed) -> syn::Result<TokenStream> {
    let calls = fields
        .named
        .iter()
        .map(|field| {
            let name = &field.ident;
            let format_str = format!("{{prefix}}{}", name.as_ref().expect("named field"));
            let description = get_field_doc(field)?;
            let options = get_parameter_options(field)?;
            let read_only = options.read_only.unwrap_or(false);
            let additional_constraints = options
                .additional_constraints
                .map_or_else(|| quote!(""), |value| quote!(#value));
            let integer_range = options.integer_range.map_or_else(
                || quote!(::std::vec::Vec::new()),
                |range| {
                    let RangeOptions { from, to, step } = range;
                    quote!(::std::vec![::r2r::IntegerRange {
                        from_value: #from,
                        to_value: #to,
                        step: #step,
                    }])
                },
            );
            let floating_point_range = options.floating_point_range.map_or_else(
                || quote!(::std::vec::Vec::new()),
                |range| {
                    let RangeOptions { from, to, step } = range;
                    quote!(::std::vec![::r2r::FloatingPointRange {
                        from_value: #from,
                        to_value: #to,
                        step: #step,
                    }])
                },
            );
            Ok(quote_spanned! {
                field.span() =>
                    let descriptor = ::r2r::ParameterDescriptor {
                        description: #description.into(),
                        additional_constraints: #additional_constraints.into(),
                        read_only: #read_only,
                        integer_range: #integer_range,
                        floating_point_range: #floating_point_range,
                        ..::std::default::Default::default()
                    };
                    self.#name.register_parameter_descriptors(
                        &format!(#format_str), Some(descriptor), descriptors
                    )?;
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! { #(#calls)* })
}

fn get_field_doc(f: &syn::Field) -> syn::Result<String> {
    let mut lines = Vec::new();
    for doc in f.attrs.iter().filter(|attr| attr.path().is_ident("doc")) {
        let value = &doc.meta.require_name_value()?.value;
        let ::syn::Expr::Lit(expr_lit) = value else {
            return Err(syn::Error::new_spanned(value, "expected a string doc comment"));
        };
        let ::syn::Lit::Str(value) = &expr_lit.lit else {
            return Err(syn::Error::new_spanned(value, "expected a string doc comment"));
        };
        lines.push(value.value().trim().to_owned());
    }
    Ok(lines.join("\n"))
}

// Generate match arms for RosParams::update_parameters()
fn param_matches_for(call: TokenStream, fields: &FieldsNamed) -> TokenStream {
    let field_matches = fields.named.iter().map(|f| {
        let name = &f.ident;
        let name_str = format!("{}", name.as_ref().expect("named field"));
        quote_spanned! {
            f.span() =>
                #name_str => {
                    self.#name.#call
                }
        }
    });
    quote! {
        #(#field_matches)*
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn tuple_struct_reports_a_compile_error() {
        let input: DeriveInput = parse_quote!(
            struct Invalid(i64);
        );
        let err = expand_r2r_params(&input).unwrap_err();

        assert_eq!(err.to_string(), "RosParams can only be derived for structs with named fields");
    }

    #[test]
    fn enum_reports_a_compile_error() {
        let input: DeriveInput = parse_quote!(
            enum Invalid {
                Value,
            }
        );
        let err = expand_r2r_params(&input).unwrap_err();

        assert_eq!(err.to_string(), "RosParams can only be derived for structs with named fields");
    }

    #[test]
    fn parameter_options_parse_ranges_read_only_and_constraints() {
        let input: DeriveInput = parse_quote! {
            struct Params {
                #[ros_param(
                    read_only,
                    additional_constraints = "calibrated",
                    integer_range(from = -4, to = 12, step = 4)
                )]
                value: i64,
            }
        };
        let Data::Struct(data) = input.data else {
            unreachable!()
        };
        let field = data.fields.iter().next().unwrap();
        let options = get_parameter_options(field).unwrap();
        assert_eq!(options.read_only, Some(true));
        assert_eq!(options.additional_constraints.unwrap().value(), "calibrated");
        let range = options.integer_range.unwrap();
        let from = range.from;
        assert_eq!(quote!(#from).to_string(), "- 4");
        assert!(options.floating_point_range.is_none());
    }

    #[test]
    fn incomplete_and_conflicting_ranges_report_compile_errors() {
        let incomplete: DeriveInput = parse_quote! {
            struct Params {
                #[ros_param(integer_range(from = 0, to = 10))]
                value: i64,
            }
        };
        assert!(expand_r2r_params(&incomplete)
            .unwrap_err()
            .to_string()
            .contains("range requires `step`"));

        let conflicting: DeriveInput = parse_quote! {
            struct Params {
                #[ros_param(
                    integer_range(from = 0, to = 10, step = 1),
                    floating_point_range(from = 0.0, to = 1.0, step = 0.1)
                )]
                value: i64,
            }
        };
        assert_eq!(
            expand_r2r_params(&conflicting).unwrap_err().to_string(),
            "integer and floating-point ranges are mutually exclusive"
        );
    }
}
