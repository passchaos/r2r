use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FieldsNamed};

extern crate proc_macro;

/// Derives RosParams trait for a structure to use it with
/// `r2r::Node::make_derived_parameter_handler()`.
#[proc_macro_derive(RosParams)]
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
}
