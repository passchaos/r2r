use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn rust_message_name<'a>(prefix: &str, name: &'a str) -> &'a str {
    if matches!(prefix, "srv" | "action") {
        name.rsplit_once('_')
            .unwrap_or_else(|| panic!("malformed {prefix} message name {name}"))
            .1
    } else {
        name
    }
}

pub(super) fn rust_message_path(module: &str, prefix: &str, name: &str) -> TokenStream {
    let module = format_ident!("{module}");
    let prefix_ident = format_ident!("{prefix}");
    if matches!(prefix, "srv" | "action") {
        let (container, leaf) = name
            .rsplit_once('_')
            .unwrap_or_else(|| panic!("malformed {prefix} message name {name}"));
        let leaf = format_ident!("{leaf}");
        if prefix == "action" {
            if let Some((action, service @ ("SendGoal" | "GetResult"))) = container.rsplit_once('_')
            {
                let action = format_ident!("{action}");
                let service = format_ident!("{service}");
                return quote! { #module :: #prefix_ident :: #action :: #service :: #leaf };
            }
        }
        let container = format_ident!("{container}");
        quote! { #module :: #prefix_ident :: #container :: #leaf }
    } else {
        let name = format_ident!("{name}");
        quote! { #module :: #prefix_ident :: #name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_message_paths_match_generated_modules() {
        for (prefix, name, expected) in [
            ("msg", "Foo_Bar", "example_interfaces::msg::Foo_Bar"),
            ("srv", "AddTwoInts_Request", "example_interfaces::srv::AddTwoInts::Request"),
            ("srv", "Foo_Bar_Response", "example_interfaces::srv::Foo_Bar::Response"),
            ("srv", "Foo_Bar_Event", "example_interfaces::srv::Foo_Bar::Event"),
            ("action", "Fibonacci_Goal", "example_interfaces::action::Fibonacci::Goal"),
            (
                "action",
                "Foo_Bar_FeedbackMessage",
                "example_interfaces::action::Foo_Bar::FeedbackMessage",
            ),
            (
                "action",
                "Fibonacci_SendGoal_Request",
                "example_interfaces::action::Fibonacci::SendGoal::Request",
            ),
            (
                "action",
                "Foo_Bar_GetResult_Response",
                "example_interfaces::action::Foo_Bar::GetResult::Response",
            ),
            (
                "action",
                "Foo_Bar_SendGoal_Event",
                "example_interfaces::action::Foo_Bar::SendGoal::Event",
            ),
            (
                "action",
                "Foo_Bar_GetResult_Event",
                "example_interfaces::action::Foo_Bar::GetResult::Event",
            ),
        ] {
            let actual: syn::Path =
                syn::parse2(rust_message_path("example_interfaces", prefix, name)).unwrap();
            let expected: syn::Path = syn::parse_str(expected).unwrap();
            assert_eq!(quote! { #actual }.to_string(), quote! { #expected }.to_string());
        }
    }

    #[test]
    fn message_names_keep_only_generated_suffixes() {
        for (prefix, name, expected) in [
            ("msg", "Foo_Bar", "Foo_Bar"),
            ("srv", "Foo_Bar_Request", "Request"),
            ("srv", "Foo_Bar_Event", "Event"),
            ("action", "Foo_Bar_FeedbackMessage", "FeedbackMessage"),
            ("action", "Foo_Bar_SendGoal_Request", "Request"),
            ("action", "Foo_Bar_GetResult_Event", "Event"),
        ] {
            assert_eq!(rust_message_name(prefix, name), expected);
        }
    }
}
