use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, ToTokens};
use syn::parse::Parser;
use syn::{Expr, Field, Ident, ItemImpl, Result, Stmt};

use crate::macros::{fallible_quote, map_or_bail};
use crate::performance::PerformanceDeclaration;
pub struct SpawningFunction {
	fun: ItemImpl,
}

impl SpawningFunction {
	pub fn new(
		actor_name: &Ident,
		data_name: &Ident,
		performances: &[PerformanceDeclaration],
	) -> Result<SpawningFunction> {
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

		let queue_constructions = map_or_bail!(
			itertools::izip!(performances, &input_field_names, &output_field_names),
			|(role, inn, out)| -> Result<Stmt> {
				let role_name = role.role_name();
				fallible_quote! { let (#inn, mut #out) = <dyn #role_name as troupe::Role>::Channel::new_default(); }
			}
		);

		let actor_fields = map_or_bail!(
			itertools::izip!(performances, &input_field_names),
			|(r, input)| -> Result<Field> {
				let field_name = r.field_name();
				Field::parse_named.parse2(fallible_quote! {#field_name : #input}?)
			}
		);

		let select_branches = map_or_bail!(
			itertools::izip!(performances, &output_field_names),
			|(role, output)| -> Result<TokenStream> {
				let fn_name = role.method_name();
				fallible_quote! { Some(msg) = #output.recv() => {
					state.#fn_name(msg)
				} }
			}
		);

		let constructor: Expr = fallible_quote! {
			#actor_name {
				#(#actor_fields),*
			}
		}?;

		let fun: ItemImpl = fallible_quote! {
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
							val.await.map_err(|_| ())?;
						}
					};
					let join_handle = ::tokio::task::spawn(event_loop);
					troupe::ActorSpawn {actor, join_handle}
				}
			}
		}?;

		Ok(SpawningFunction { fun })
	}
}

impl ToTokens for SpawningFunction {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.fun.to_tokens(tokens);
	}
}
