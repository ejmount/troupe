use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_quote, ItemImpl};

use crate::performance::PerformanceDeclaration;

pub struct InfoType {
	impl_block: ItemImpl,
}

impl InfoType {
	pub fn new(perf: &PerformanceDeclaration) -> InfoType {
		let payload_name = perf.payload_name();
		let role_name = perf.role_name();

		InfoType {
			impl_block: parse_quote! {
				impl troupe::Role for dyn #role_name {
					type Payload = #payload_name;
					type Channel = troupe::tokio::TokioUnbounded<Self::Payload>;
				}
			},
		}
	}
}

impl ToTokens for InfoType {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.impl_block.to_tokens(tokens);
	}
}
