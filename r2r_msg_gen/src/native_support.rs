//! Generated safe adapters validate lazy interface operations before returning native data.
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

pub fn typesupport(getter: &Ident, support: &Ident, interface: &str) -> TokenStream {
    let helper = format_ident!("r2r_resolve_{getter}");
    quote! {
        #[inline]
        fn get_ts()->&'static #support {
            Self::try_get_ts().expect("native ROS type support unavailable")
        }
        #[inline]
        fn try_get_ts()->crate::Result<&'static #support> {
            #[cfg(not(feature="doc-only"))]
            let pointer={
                let operation=r2r_msg_gen::#helper().map_err(|error|crate::Error::NativeTypeSupport{interface:#interface.into(),err:error.to_string()})?;
                unsafe{operation()}
            };
            #[cfg(feature="doc-only")]
            let pointer=#getter();
            unsafe{pointer.as_ref()}.ok_or_else(||crate::Error::NativeTypeSupport{interface:#interface.into(),err:"type support returned a null handle".into()})
        }
    }
}

pub fn message(
    init: &Ident, fini: &Ident, create: &Ident, destroy: &Ident, c_struct: &Ident, interface: &str,
) -> TokenStream {
    let init_helper = format_ident!("r2r_resolve_{init}");
    let fini_helper = format_ident!("r2r_resolve_{fini}");
    let create_helper = format_ident!("r2r_resolve_{create}");
    let destroy_helper = format_ident!("r2r_resolve_{destroy}");
    quote! {
        #[inline]
        fn create_msg()->*mut #c_struct {Self::try_create_msg().unwrap_or(std::ptr::null_mut())}
        #[inline]
        fn try_create_msg()->crate::Result<*mut #c_struct> {
            #[cfg(not(feature="doc-only"))]
            {
                // Resolve destruction first: every successful owned allocation
                // has a process-retained matching finalizer before creation.
                r2r_msg_gen::#destroy_helper().map_err(|error|crate::Error::NativeTypeSupport{interface:#interface.into(),err:error.to_string()})?;
                let operation=r2r_msg_gen::#create_helper().map_err(|error|crate::Error::NativeTypeSupport{interface:#interface.into(),err:error.to_string()})?;
                Ok(unsafe{operation()})
            }
            #[cfg(feature="doc-only")]
            Ok(#create())
        }
        #[inline]
        fn destroy_msg(message:*mut #c_struct) {
            #[cfg(not(feature="doc-only"))]
            {
                let operation=r2r_msg_gen::#destroy_helper().expect("native finalizer must remain available after successful creation");
                unsafe{operation(message)};
            }
            #[cfg(feature="doc-only")]
            #destroy(message);
        }
        #[cfg(not(feature="doc-only"))]
        #[inline]
        fn try_reinitialize_native(message:&mut #c_struct)->crate::Result<bool> {
            // Resolve both operations before touching the current value. Initialize a temporary
            // first so allocation failure also leaves the current message intact and destructible.
            let finalize=r2r_msg_gen::#fini_helper().map_err(|error|crate::Error::NativeTypeSupport{interface:#interface.into(),err:error.to_string()})?;
            let initialize=r2r_msg_gen::#init_helper().map_err(|error|crate::Error::NativeTypeSupport{interface:#interface.into(),err:error.to_string()})?;
            unsafe {crate::msg_types::try_reinitialize_native_message(message,initialize,finalize)?};
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_creation_resolves_finalizer_first_and_safe_typesupport_checks_null() {
        let create = format_ident!("p__msg__M__create");
        let destroy = format_ident!("p__msg__M__destroy");
        let init = format_ident!("p__msg__M__init");
        let fini = format_ident!("p__msg__M__fini");
        let c_struct = format_ident!("p__msg__M");
        let tokens = message(&init, &fini, &create, &destroy, &c_struct, "p/msg/M").to_string();
        assert!(
            tokens.find("r2r_resolve_p__msg__M__destroy").unwrap()
                < tokens.find("r2r_resolve_p__msg__M__create").unwrap()
        );
        assert!(tokens.contains("NativeTypeSupport"));
        assert!(tokens.contains("try_reinitialize_native"));
        assert!(tokens.contains("try_reinitialize_native_message"));
        let tokens =
            typesupport(&format_ident!("get_ts"), &format_ident!("Support"), "p/msg/M").to_string();
        assert!(tokens.contains("as_ref"));
        assert!(tokens.contains("null handle"));
        assert!(!tokens.contains("& *"));
    }
}
