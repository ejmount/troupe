#![warn(clippy::pedantic)]
#![allow(clippy::trivially_copy_pass_by_ref)]

mod actor_decl;
use actor_decl::Role;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::Span;
use proc_macro2::TokenStream;

use quote::ToTokens;

use quote::format_ident;
use syn::Error;
use syn::Item;
use syn::ItemEnum;
use syn::ItemImpl;
use syn::ItemMod;
use syn::ItemStruct;
use syn::ItemUnion;

macro_rules! make_filter {
    ($pat:path) => {
        |thing| if let $pat(a) = thing { Some(a) } else { None }
    };
}

fn merge_errors(mut e: Error, f: Error) -> Error {
    e.combine(f);
    e
}

fn make_role(role: &ItemImpl) -> Result<Role, Error> {
    let Some((_, ref path, _)) = role.trait_ else { return Err(Error::new_spanned(role, "Must impl a role trait")) };

    Ok(Role::new(path.clone()))
}

fn get_data_item(items: &[Item]) -> Result<Item, Error> {
    let mut data_items = items
        .into_iter()
        .filter(|i| matches!(i, Item::Struct(_) | Item::Enum(_) | Item::Union(_)));

    let Some(first_data) = data_items.next() else { return Err(Error::new(Span::call_site(), "actor declaration must contain one struct, enum or union")); };

    let invalid_items = data_items
        .map(|item| Error::new_spanned(item, "Only one data item allowed in actor declaration"));

    let error = invalid_items.reduce(merge_errors);

    match error {
        Some(err) => Err(err),
        None => Ok(first_data.clone()),
    }
}

fn actor_core(module: ItemMod) -> Result<Vec<Item>, syn::Error> {
    let Some((_, items)) = &module.content else { return Err(Error::new_spanned(module, "actor declaration cannot be empty")); };

    let data_item = get_data_item(&items)?;

    let roles = items.iter().filter_map(make_filter!(Item::Impl));

    let (roles, errors): (Vec<_>, _) = roles.map(make_role).partition(Result::is_ok);

    let role_errors = errors
        .into_iter()
        .map(Result::unwrap_err)
        .reduce(merge_errors);
    if let Some(err) = role_errors {
        return Err(err);
    }

    let data_name = match &data_item {
        Item::Struct(ItemStruct { ident, .. })
        | Item::Enum(ItemEnum { ident, .. })
        | Item::Union(ItemUnion { ident, .. }) => ident,
        _ => unreachable!(),
    };

    let actor_name = format_ident!("{data_name}Actor");

    let roles: Vec<_> = roles.into_iter().map(Result::unwrap).collect();

    let actor_impls = actor_decl::create_actor_impls(&roles[..], &actor_name)
        .into_iter()
        .map(Item::Impl);
    let actor_type = actor_decl::create_actor_type(&roles[..], &actor_name);
    let trait_decl = roles.iter().flat_map(actor_decl::create_role);

    let payloads = roles
        .iter()
        .map(actor_decl::create_payload)
        .map(Item::Struct);

    let mut output = vec![Item::Struct(actor_type)];
    output.extend(actor_impls);
    output.extend(trait_decl);
    output.extend(payloads);
    return Ok(output);
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
