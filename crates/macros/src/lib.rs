use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, ItemTrait, parse_macro_input};

#[proc_macro_attribute]
pub fn gen_async_version(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(item as ItemTrait);

    // Extract the trait name
    let trait_name = &input.ident;
    let async_trait_name = syn::Ident::new(&format!("{}Async", trait_name), trait_name.span());

    // Generate async methods
    let async_methods = input.items.iter().filter_map(|item| {
        if let syn::TraitItem::Fn(method) = item {
            let sig = &method.sig;
            let async_sig = syn::Signature {
                asyncness: Some(Default::default()), // Add async keyword
                ..sig.clone()
            };

            let docs: Vec<&Attribute> = method
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .collect();

            Some(quote! {
                #(#docs)*
                #async_sig;
            })
        } else {
            None
        }
    });

    // Generate the new async trait
    let expanded = quote! {
        #input

        pub trait #async_trait_name {
            #(#async_methods)*
        }
    };

    // Convert the expanded code into a TokenStream and return it
    TokenStream::from(expanded)
}
