use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::{parse_quote, Expr, Field, Ident, ItemImpl, Stmt};

use crate::performance::PerformanceDeclaration;
pub struct SpawningFunction {
	fun: ItemImpl,
}

impl SpawningFunction {
	pub fn new(
		actor_name: &Ident,
		data_name: &Ident,
		performances: &[PerformanceDeclaration],
	) -> SpawningFunction {
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
				|(role, inn, out)| -> Stmt {
					let role_name = role.role_name();
					parse_quote! { let (#inn, mut #out) = <dyn #role_name as troupe::Role>::Channel::new_default(); }
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
					state.#fn_name(msg)
				} }
			},
		);

		let constructor: Expr = parse_quote! {
			#actor_name {
				#(#actor_fields),*
			}
		};

		SpawningFunction {
			fun: parse_quote! {
				impl #data_name {
					pub fn start(mut state: #data_name) -> troupe::ActorSpawn<#actor_name> {
						use troupe::Channel;
						#(#queue_constructions)*
						let actor = ::std::sync::Arc::new(#constructor);
						let event_loop = async move {
							loop {
								let val = ::tokio::select! {
									#(#select_branches),*
								};
								val.map_err(|_| ())?;
							}
						};
						let join_handle = ::tokio::task::spawn(event_loop);
						troupe::ActorSpawn {actor, join_handle}
					}
				}
			},
		}
	}
}

impl ToTokens for SpawningFunction {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.fun.to_tokens(tokens);
	}
}
