use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, ToTokens};
use syn::fold::Fold;
use syn::{parse_quote, FnArg, ItemEnum, ItemTrait, Signature, Variant};

use crate::infotype::InfoType;
use crate::namerewriter::NameRewriter;
use crate::performance::PerformanceDeclaration;

macro_rules! filter_unwrap {
	($list:expr, $pat:path) => {
		$list
			.iter()
			.cloned()
			.filter_map(|item| if let $pat(a) = item { Some(a) } else { None })
	};
}

pub struct Role {
	info:    InfoType,
	payload: ItemEnum,
	trt:     ItemTrait,
}

impl Role {
	pub fn new(perf: &PerformanceDeclaration) -> Role {
		let info = InfoType::new(perf);
		let trait_name = perf.role_name();

		let signatures = perf.handlers().iter().map(|i| &i.sig).collect_vec();
		let payload = create_payload_from_impl(&perf.payload_name(), &signatures);

		let signatures = signatures.into_iter();

		let trt = parse_quote! {
			trait #trait_name {
				type Info: troupe::RoleInfo;
				#(#signatures;)*
			}
		};

		let trt = NameRewriter::new(perf.role_name()).fold_item_trait(trt);

		Role { info, payload, trt }
	}
}

impl ToTokens for Role {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.info.to_tokens(tokens);
		self.payload.to_tokens(tokens);
		self.trt.to_tokens(tokens);
	}
}

fn make_variant(sig: &Signature) -> Variant {
	let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

	let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|pat| *pat.ty);
	parse_quote! { #variant_name ((#(#types),*)) }
}

fn create_payload_from_impl(payload_name: &Ident, methods: &[&Signature]) -> ItemEnum {
	let variants = methods.iter().copied().map(make_variant);

	parse_quote! {
		pub enum #payload_name { #(#variants),* }
	}
}