use itertools::{Either, Itertools};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::{
	Attribute, Error, Field, Ident, Item, ItemEnum, ItemImpl, ItemStruct, ItemUnion, Result,
};

use crate::macros::{fallible_quote, map_or_bail};
use crate::performance::PerformanceDeclaration;
use crate::role::Role;
use crate::spawning_function::SpawningFunction;
use crate::Performance;

pub struct ActorDecl {
	actor_struct:                ItemStruct,
	other_items:                 Vec<Item>,
	performance_implementations: Vec<Performance>,
	roles:                       Vec<Role>,
	spawner:                     SpawningFunction,
}

impl ActorDecl {
	pub fn new(items: &[Item]) -> Result<ActorDecl> {
		let data_name = get_data_item_name(items)?;
		let actor_name = format_ident!("{}Actor", data_name);

		let (performance_items, other_items): (Vec<_>, Vec<_>) =
			items.iter().cloned().partition_map(|i| match i {
				Item::Impl(imp) if get_performance_tag(&imp).is_some() => Either::Left(imp),
				other => Either::Right(other),
			});

		let performance_names = performance_items
			.iter()
			.map(|i| &i.trait_.as_ref().unwrap().1);

		let perf_attributes = map_or_bail!(&performance_items, |i| get_performance_tag(i)
			.unwrap()
			.parse_args());

		let performances = itertools::izip!(performance_names, &performance_items, perf_attributes)
			.map(|(role_name, imp, attribute)| {
				PerformanceDeclaration::new(role_name, imp, attribute)
			})
			.collect_vec();

		let spawner = SpawningFunction::new(&actor_name, &data_name, &performances)?;

		let actor_struct = make_actor_struct(&actor_name, &performances)?;

		let performance_implementations = map_or_bail!(&performances, |perf| Performance::new(
			&actor_name,
			&data_name,
			&perf
		));

		let roles = map_or_bail!(&performances, make_canonical_role)
			.into_iter()
			.flatten()
			.collect_vec();

		Ok(ActorDecl {
			actor_struct,
			other_items,
			performance_implementations,
			roles,
			spawner,
		})
	}
}

impl ToTokens for ActorDecl {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		use quote::TokenStreamExt;
		self.actor_struct.to_tokens(tokens);
		tokens.append_all(&self.other_items);
		self.spawner.to_tokens(tokens);
		tokens.append_all(&self.roles);
		tokens.append_all(&self.performance_implementations);
	}
}

fn get_data_item_name(items: &[Item]) -> Result<Ident> {
	let data_items = items.iter().filter_map(|item| match item.clone() {
		Item::Struct(ItemStruct { ident, .. })
		| Item::Enum(ItemEnum { ident, .. })
		| Item::Union(ItemUnion { ident, .. }) => Some((item, ident)),
		_ => None,
	});

	match data_items.at_most_one() {
		Ok(Some((_, first_ident))) => Ok(first_ident),
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

fn merge_errors(mut e: Error, f: Error) -> Error {
	e.combine(f);
	e
}

fn make_actor_struct(
	actor_name: &Ident,
	performances: &[PerformanceDeclaration],
) -> Result<ItemStruct> {
	let fields = map_or_bail!(performances, make_field_from_name);

	fallible_quote! {
		struct #actor_name {
			#(#fields),*
		}
	}
}

fn make_field_from_name(performance: &PerformanceDeclaration) -> Result<Field> {
	let field_name = performance.field_name();
	let role_name = performance.role_name();

	Field::parse_named
		.parse2(
			quote! {#field_name : <<dyn #role_name as troupe::Role>::Channel as troupe::Channel>::Sender},
		)
		.map_err(|err| {
			syn::parse::Error::new(err.span(),
				format!("Parse failure trying to create actor field: {err} - this is a bug, please file an issue")
			)
		})
}

fn make_canonical_role(perf_decl: &PerformanceDeclaration) -> Result<Option<Role>> {
	if perf_decl.attribute().canonical {
		Ok(Some(Role::new(perf_decl)?))
	} else {
		Ok(None)
	}
}

fn get_performance_tag(imp: &ItemImpl) -> Option<&Attribute> {
	imp.attrs
		.iter()
		.find(|attr| attr.path().is_ident("performance"))
}
