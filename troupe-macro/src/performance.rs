use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, ToTokens};
use syn::fold::Fold;
use syn::parse::Parser;
use syn::{Arm, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Path, Result, Signature};

use crate::attributes::PerformanceAttribute;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};
use crate::namerewriter::MethodRewriter;

pub struct PerformanceDeclaration {
	role_name: Path,
	attribute: PerformanceAttribute,
	handlers:  Vec<ImplItemFn>,
}

impl PerformanceDeclaration {
	pub fn new(
		role_name: &Path,
		imp: &ItemImpl,
		attribute: PerformanceAttribute,
	) -> PerformanceDeclaration {
		assert!(!role_name.segments.is_empty());
		let handlers = filter_unwrap! {imp.items.clone(), ImplItem::Fn}.collect_vec();

		PerformanceDeclaration {
			role_name: role_name.clone(),
			attribute,
			handlers,
		}
	}

	fn leaf_ident(&self) -> String {
		self.role_name
			.segments
			.last()
			.as_ref()
			.unwrap()
			.ident
			.to_string()
	}

	pub fn attribute(&self) -> &PerformanceAttribute {
		&self.attribute
	}

	pub fn handlers(&self) -> &[ImplItemFn] {
		&self.handlers
	}

	pub fn role_name(&self) -> Path {
		self.role_name.clone()
	}

	pub fn payload_name(&self) -> Ident {
		format_ident!("{}Payload", self.leaf_ident().to_case(Case::UpperCamel))
	}

	pub fn method_name(&self) -> Ident {
		let role_name = self.leaf_ident();
		format_ident!("{}", format!("perform_{role_name}").to_case(Case::Snake))
	}

	pub fn field_name(&self) -> Ident {
		format_ident!("{}", self.leaf_ident().to_case(Case::Snake))
	}
}

pub struct Performance {
	data_impl:  ItemImpl,
	actor_impl: ItemImpl,
}

impl Performance {
	pub fn new(
		actor_name: &Ident,
		data_name: &Ident,
		perf: &PerformanceDeclaration,
	) -> Result<Performance> {
		let data_impl = make_data_performance(data_name, perf)?;
		let actor_impl = make_actor_performance(actor_name, perf)?;
		Ok(Performance {
			data_impl,
			actor_impl,
		})
	}
}

impl ToTokens for Performance {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.data_impl.to_tokens(tokens);
		self.actor_impl.to_tokens(tokens);
	}
}

fn make_actor_performance(actor_name: &Ident, perf: &PerformanceDeclaration) -> Result<ItemImpl> {
	let methods = map_or_bail!(&perf.handlers, |fun| sending_method(perf, &fun));

	let trait_name = perf.role_name();

	let output = fallible_quote! {
		#[::async_trait::async_trait]
		impl #trait_name for #actor_name {
			#(#methods)*
		}
	}?;

	Ok(MethodRewriter::new(perf.role_name()).fold_item_impl(output))
}

fn sending_method(perf: &PerformanceDeclaration, fun: &ImplItemFn) -> Result<ImplItemFn> {
	let payload_name = perf.payload_name();
	let field_name = perf.field_name();

	let params = (0..fun.sig.inputs.len() - 1).map(|n| format_ident!("_{n}"));
	let variant_name = make_variant_name(fun);
	let sig = Signature {
		asyncness: None,
		..fun.sig.clone()
	};

	fallible_quote! {
		async #sig {
			use troupe::{RoleReceiver, RoleSender};
			let msg = (#(#params),*);
			let field: &dyn troupe::RoleSender::<#payload_name, Error = _> = &self.#field_name;
			field.send(#payload_name::#variant_name(msg)).await
		}
	}
}

fn make_data_performance(data_name: &Ident, perf: &PerformanceDeclaration) -> Result<ItemImpl> {
	let payload_name = perf.payload_name();
	let method_name = perf.method_name();

	let arms: Result<Vec<_>> = perf
		.handlers
		.iter()
		.map(|fun| -> Result<Arm> {
			let patterns = filter_unwrap!(fun.sig.inputs.clone(), FnArg::Typed).map(|p| *p.pat);

			let variant_name = make_variant_name(fun);

			let body = &fun.block;
			fallible_quote! {
				#payload_name::#variant_name ((#(#patterns),*)) => #body,
			}
		})
		.collect();

	let arms = arms?;

	fallible_quote! {
		impl #data_name {
			async fn #method_name(&mut self, msg: #payload_name) -> Result<(), ()> {
				let val = match msg {
					#(#arms),*
				};
				Ok(val)
			}
		}
	}
}

fn make_variant_name(function: &ImplItemFn) -> Ident {
	let name = function.sig.ident.to_string();
	format_ident!("{}", name.to_case(Case::UpperCamel))
}
