use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, ToTokens};
use syn::fold::Fold;
use syn::parse::Parser;
use syn::{FnArg, ItemEnum, ItemTrait, PatType, Result, Signature, Variant};

use crate::infotype::InfoType;
use crate::macros::{fallible_quote, map_or_bail};
use crate::namerewriter::MethodRewriter;
use crate::performance::PerformanceDeclaration;

pub struct Role {
	info:    InfoType,
	payload: ItemEnum,
	trt:     ItemTrait,
}

impl Role {
	pub fn new(perf: &PerformanceDeclaration) -> Result<Role> {
		let info = InfoType::new(perf)?;
		let trait_name = perf.role_name();

		let signatures = perf.handlers().iter().map(|i| &i.sig).collect_vec();
		let payload = create_payload_from_impl(&perf.payload_name(), &signatures)?;

		let signatures = signatures.into_iter();

		let trt = fallible_quote! {
			trait #trait_name {
				#(#signatures;)*
			}
		}?;

		let trt = MethodRewriter::new(perf.role_name()).fold_item_trait(trt);

		let trt = fallible_quote! {
			#[::async_trait::async_trait]
			#trt
		}?;

		Ok(Role { info, payload, trt })
	}
}

impl ToTokens for Role {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.info.to_tokens(tokens);
		self.payload.to_tokens(tokens);
		self.trt.to_tokens(tokens);
	}
}

fn create_payload_from_impl(payload_name: &Ident, methods: &[&Signature]) -> Result<ItemEnum> {
	fn make_variant(sig: &Signature) -> Result<Variant> {
		let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

		let types = sig.inputs.iter().filter_map(|item| {
			if let FnArg::Typed(PatType { ty, .. }) = item {
				Some(ty)
			} else {
				None
			}
		});
		fallible_quote! { #variant_name ((#(#types),*)) }
	}
	let variants = map_or_bail!(methods.iter().cloned(), make_variant);

	fallible_quote! {
		pub enum #payload_name { #(#variants),* }
	}
}
