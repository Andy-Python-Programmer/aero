use proc_macro::TokenStream;
use syn::{Lit, Meta, MetaNameValue, NestedMeta};

pub fn parse(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemStatic);

    let ty = &item.ty;
    let ident = &item.ident;
    let mutability = &item.mutability;
    let vis = &item.vis;
    let initializer = &item.expr;

    // Parse the attribute arguments
    let args = syn::parse_macro_input!(attr as syn::AttributeArgs);

    // Process each argument to find the subsection value
    let mut subsection = None;
    for arg in args {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) = arg {
            if let Some(ident) = path.get_ident() {
                if ident == "subsection" {
                    if let Lit::Str(lit_str) = lit {
                        subsection = Some(lit_str.value());
                    }
                }
            }
        }
    }

    let link_section = match subsection {
        Some(subsection) => format!(".cpu_local_{}", subsection),
        None => ".cpu_local".to_string(),
    };

    quote::quote! {
        #[link_section = #link_section]
        #[used]
        #vis static #mutability #ident: crate::arch::cpu_local::CpuLocal<#ty> = crate::arch::cpu_local::CpuLocal::new(#initializer);
    }
    .into()
}
