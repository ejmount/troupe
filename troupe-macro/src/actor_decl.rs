use itertools::{Either, Itertools};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::{
	parse_quote, Attribute, Error, Expr, Field, Ident, Item, ItemEnum, ItemImpl, ItemStruct,
	ItemUnion, Stmt,
};

use crate::performance::PerformanceDeclaration;
use crate::role::Role;
use crate::Performance;

pub struct ActorDecl {
	actor_struct:                ItemStruct,
	other_items:                 Vec<Item>,
	performance_implementations: Vec<Performance>,
	roles:                       Vec<Role>,
	spawner:                     ItemImpl,
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

		let spawner = make_spawner(&actor_name, &data_name, &performances);

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

fn make_spawner(
	actor_name: &Ident,
	data_name: &Ident,
	performances: &[PerformanceDeclaration],
) -> ItemImpl {
	let field_names = performances
		.iter()
		.map(PerformanceDeclaration::field_name)
		.collect_vec();

	let input_field_names = field_names
		.iter()
		.map(|name| format_ident!("{}_input", name))
		.collect_vec();

	let output_field_names = field_names
		.iter()
		.map(|name| format_ident!("{}_output", name))
		.collect_vec();

	let queue_constructions =
		itertools::izip!(performances, &input_field_names, &output_field_names).map(
			|(_role, inn, out)| -> Stmt {
				let constructor: Expr = parse_quote! { ::tokio::sync::mpsc::unbounded_channel() };
				parse_quote! { let (#inn, mut #out) = #constructor; }
			},
		);

	let actor_fields = itertools::izip!(performances, &input_field_names)
        .map(|(r, input)| -> Field {
            let field_name = r.field_name();
            Field::parse_named
            .parse2(quote! {#field_name : #input})
            .unwrap_or_else(|_| panic!("Parse failure trying to create actor field {actor_name} - this is a bug, please file an issue"))
        });

	let select_branches = itertools::izip!(performances, output_field_names.iter()).map(
		|(role, output)| -> TokenStream {
			let fn_name = role.method_name();
			quote! { Some(msg) = #output.recv() => {
				state.#fn_name(msg);
			} }
		},
	);

	parse_quote! {
		impl #data_name {
			pub fn start(mut state: #data_name) -> (::std::sync::Arc<#actor_name>, ::tokio::task::JoinHandle<()>) {
				#(#queue_constructions)*
				let actor = #actor_name {
					#(#actor_fields),*
				};
				let actor_handle = ::std::sync::Arc::new(actor);
				let event_loop = async move {
					loop {
						::tokio::select! {
							#(#select_branches),*
							else => break
						}
					}
				};
				let join_handle = ::tokio::task::spawn(event_loop);
				(actor_handle, join_handle)
			}
		}
	}
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
		.parse2(quote! {#field_name : <<Self as #role_name>::Info as troupe::RoleInfo>::Sender})
		.unwrap_or_else(|_| {
			panic!(
				"Parse failure trying to create actor field - this is a bug, please file an issue"
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
