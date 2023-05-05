use itertools::{Either, Itertools};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::{
	parse_quote, Attribute, Error, Field, Ident, Item, ItemEnum, ItemImpl, ItemStruct, ItemUnion,
};

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
	pub fn new(items: &[Item]) -> Result<ActorDecl, Error> {
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

		let perf_attributes = performance_items
			.iter()
			.map(|i| get_performance_tag(i).unwrap().parse_args())
			.collect::<Result<Vec<_>, _>>()?;

		let performances = itertools::izip!(performance_names, &performance_items, perf_attributes)
			.map(|(role_name, imp, attribute)| {
				PerformanceDeclaration::new(role_name, imp, attribute)
			})
			.collect_vec();

		let spawner = SpawningFunction::new(&actor_name, &data_name, &performances);

		let actor_struct = make_actor_struct(&actor_name, &performances);

		let performance_implementations = performances
			.iter()
			.map(|perf| Performance::new(&actor_name, &data_name, perf))
			.collect_vec();

		let roles = performances
			.iter()
			.filter_map(make_canonical_role)
			.collect();

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
		self.actor_struct.to_tokens(tokens);
		for imp in &self.other_items {
			imp.to_tokens(tokens);
		}
		self.spawner.to_tokens(tokens);
		for role in &self.roles {
			role.to_tokens(tokens);
		}
		for imp in &self.performance_implementations {
			imp.to_tokens(tokens);
		}
	}
}

fn get_data_item_name(items: &[Item]) -> Result<Ident, Error> {
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

fn make_actor_struct(actor_name: &Ident, performances: &[PerformanceDeclaration]) -> ItemStruct {
	let fields = performances.iter().map(make_field_from_name);

	parse_quote! {
		struct #actor_name {
			#(#fields),*
		}
	}
}

fn make_field_from_name(performance: &PerformanceDeclaration) -> Field {
	let field_name = performance.field_name();
	let role_name = performance.role_name();

	Field::parse_named
		.parse2(
			quote! {#field_name : <<dyn #role_name as troupe::Role>::Channel as troupe::Channel>::Sender},
		)
		.unwrap_or_else(|err| {
			panic!(
				"Parse failure trying to create actor field - this is a bug, please file an issue: {err}"
			)
		})
}

fn make_canonical_role(perf_decl: &PerformanceDeclaration) -> Option<Role> {
	perf_decl
		.attribute()
		.canonical
		.then(|| Role::new(perf_decl))
}

fn get_performance_tag(imp: &ItemImpl) -> Option<&Attribute> {
	imp.attrs
		.iter()
		.find(|attr| attr.path().is_ident("performance"))
}
