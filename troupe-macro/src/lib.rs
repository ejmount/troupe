#![warn(clippy::pedantic)]
#![allow(clippy::trivially_copy_pass_by_ref)]

#[macro_use]
mod actor_decl;
mod types;

use itertools::Itertools;
use types::Performance;
use types::Role;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::Span;
use proc_macro2::TokenStream;

use quote::ToTokens;

use quote::format_ident;
use syn::Error;
use syn::Ident;
use syn::Item;
use syn::ItemEnum;
use syn::ItemImpl;
use syn::ItemMod;
use syn::ItemStruct;
use syn::ItemUnion;

use crate::actor_decl::create_handling_block;
use crate::actor_decl::create_performance;
use crate::actor_decl::create_spawner;

fn merge_errors(mut e: Error, f: Error) -> Error {
    e.combine(f);
    e
}

fn create_role(role: ItemImpl) -> Result<Role, Error> {
    let Some((_, path, _)) = role.trait_ else { return Err(Error::new_spanned(role, "Must impl a role trait")) };

    Ok(Role::new(path))
}

fn make_performed_role(imp: ItemImpl) -> Result<(Role, Performance), Error> {
    let role = create_role(imp.clone())?;

    let performance = create_performance(imp)?;
    Ok((role, performance))
}

fn get_data_item_name(items: &[Item]) -> Result<(&Item, &Ident), Error> {
    let data_items = items.iter().filter_map(|item| match item {
        Item::Struct(ItemStruct { ident, .. })
        | Item::Enum(ItemEnum { ident, .. })
        | Item::Union(ItemUnion { ident, .. }) => Some((item, ident)),
        _ => None,
    });

    match data_items.at_most_one() {
        Ok(Some(first_item)) => Ok(first_item),
        Ok(None) => Err(Error::new(
            Span::call_site(),
            "actor declaration must contain one struct, enum or union",
        )),
        Err(items) => Err(items
            .map(|(item, _)| {
                Error::new_spanned(item, "Only one data item allowed in actor declaration")
            })
            .reduce(merge_errors)
            .unwrap()),
    }
}

fn actor_core(module: ItemMod) -> Result<Vec<Item>, syn::Error> {
    let Some((_brace, items)) = module.content else { return Err(Error::new_spanned(module, "actor declaration cannot be empty")); };

    let (data_item, data_name) = get_data_item_name(&items)?;
    let (data_item, data_name) = (data_item.clone(), data_name.clone());

    let roles = filter_unwrap!(items, Item::Impl);

    let (performances, inherents): (Vec<_>, _) = roles.partition(|imp| imp.trait_.is_some());

    let inherents = inherents.into_iter().map(Item::Impl);

    let (performed_roles, role_errors): (Vec<_>, Vec<_>) = performances
        .into_iter()
        .map(make_performed_role)
        .partition_result();

    bail_if_any!(role_errors);

    let actor_name = format_ident!("{data_name}Actor");

    let roles: Vec<_> = performed_roles
        .iter()
        .map(|(role, _)| role.clone())
        .collect_vec();

    let actor_impls = actor_decl::create_actor_impls(&roles[..], &actor_name)
        .into_iter()
        .map(Item::Impl);
    let actor_type = actor_decl::create_actor_type(&roles[..], &actor_name);
    let trait_decl = roles.iter().flat_map(actor_decl::create_role);

    let payloads = performed_roles
        .iter()
        .map(|(a, p)| actor_decl::create_payload(a, p))
        .map(Item::Enum);

    let handling = Item::Impl(create_handling_block(&data_name, &performed_roles));

    let spawner = Item::Impl(create_spawner(&data_name, &actor_name, &roles));

    let mut output = vec![Item::Struct(actor_type)];
    output.push(data_item);
    output.extend(inherents);
    output.extend(actor_impls);
    output.extend(trait_decl);
    output.extend(payloads);
    output.push(handling);
    output.push(spawner);
    Ok(output)
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
