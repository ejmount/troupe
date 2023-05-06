use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, ToTokens};
use syn::fold::Fold;
use syn::parse::Parser;
use syn::{FnArg, ItemEnum, ItemImpl, ItemTrait, Result, Signature, Variant};

use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};
use crate::namerewriter::MethodRewriter;
use crate::performance::PerformanceDeclaration;

pub struct Role {
	trait_def:  ItemTrait,
	trait_impl: ItemImpl,
	payload:    ItemEnum,
}

impl Role {
	pub fn new(perf: &PerformanceDeclaration) -> Result<Role> {
		let role_name = perf.role_name();

		let signatures = perf.handlers().into_iter().map(|i| &i.sig).collect_vec();
		let payload = create_payload_from_impl(&perf.payload_name(), signatures.clone())?;

		let trt = fallible_quote! {
			trait #role_name {
				#(#signatures;)*
			}
		}?;

		let trt = MethodRewriter::new(role_name).fold_item_trait(trt);

		let trait_def = fallible_quote! {
			#[::async_trait::async_trait]
			#trt
		}?;

		let trait_impl = {
			let perf = perf;
			let payload_name = perf.payload_name();
			let role_name = perf.role_name();

			fallible_quote! {
				impl troupe::Role for dyn #role_name {
					type Payload = #payload_name;
					type Channel = troupe::tokio::TokioUnbounded<Self::Payload>;
				}
			}
		}?;

		Ok(Role {
			payload,
			trait_def,
			trait_impl,
		})
	}
}

impl ToTokens for Role {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.trait_impl.to_tokens(tokens);
		self.payload.to_tokens(tokens);
		self.trait_def.to_tokens(tokens);
	}
}

fn create_payload_from_impl(payload_name: &Ident, methods: Vec<&Signature>) -> Result<ItemEnum> {
	fn make_variant(sig: &Signature) -> Result<Variant> {
		let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

		let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|p| &*p.ty);
		fallible_quote! { #variant_name ((#(#types),*)) }
	}
	let variants = map_or_bail!(methods, make_variant);

	fallible_quote! {
		pub enum #payload_name { #(#variants),* }
	}
}
