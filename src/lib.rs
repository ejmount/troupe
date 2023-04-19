#![warn(clippy::pedantic)]

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::TokenStream;

use quote::ToTokens;

use syn::Error;
use syn::Item;
use syn::ItemMod;

fn check_for_extra_data_items(items: Vec<Item>) -> Option<Error> {
    let mut invalid_data_items = items
        .iter()
        .filter(|i| matches!(i, Item::Struct(_) | Item::Enum(_) | Item::Union(_)))
        .skip(1)
        .map(|item| Error::new_spanned(item, "Only one data item allowed in actor declaration"));

    let mut error = invalid_data_items.next();
    for item in invalid_data_items {
        if let Some(mut err) = error {
            err.combine(item);
            error = Some(err);
        }
    }
    error
}

fn actor_core(module: ItemMod) -> Result<Vec<Item>, syn::Error> {
    let Some((_, items)) = module.content else { return Err(syn::Error::new_spanned(module, "actor declaration cannot be empty")); };

    if let Some(value) = check_for_extra_data_items(items) {
        return Err(value);
    }

    Ok(vec![])
}

#[proc_macro_attribute]
pub fn actor(_attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let module = syn::parse_macro_input!(item as ItemMod);

    match actor_core(module) {
        Ok(items) => {
            let mut token_stream = TokenStream::new();
            for item in items {
                item.to_tokens(&mut token_stream);
            }
            token_stream.into()
        }
        Err(e) => e.into_compile_error().into(),
    }
}
