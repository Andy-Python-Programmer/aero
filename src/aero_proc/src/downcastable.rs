use proc_macro::TokenStream;
use syn::spanned::Spanned;

pub fn parse(_: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_trait = syn::parse_macro_input!(item as syn::ItemTrait);

    let vis = &parsed_trait.vis;
    let name = &parsed_trait.ident;
    let items = &parsed_trait.items;
    let generics = &parsed_trait.generics;

    // `auto` and `unsafe` traits are not allowed:
    if let Some(token) = parsed_trait.auto_token {
        emit_error!(token.span(), "`auto` traits are not downcastable")
    } else if let Some(token) = parsed_trait.unsafety {
        emit_error!(token.span(), "`unsafe` traits are not downcastable")
    }

    let super_traits = parsed_trait.supertraits.clone();

    quote::quote! {
        #vis trait #name #generics: #super_traits + crate::utils::Downcastable {
            #(#items)*
        }

        // #[downcast]: implement downcast functions:
        impl dyn #name #generics {
            /// Downcast's an `Arc`ed trait object to an `Arc`ed object if the underlying object
            /// is of type `T`.
            pub fn downcast_arc<T: #name #generics>(self: &alloc::sync::Arc<Self>) -> Option<alloc::sync::Arc<T>> {
                self.clone().as_any().downcast::<T>().ok()
            }
        }
    }
    .into()
}
