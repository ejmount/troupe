use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_quote, ItemImpl, ItemStruct};

use crate::performance::PerformanceDeclaration;

pub struct InfoType {
	definition: ItemStruct,
	impl_block: ItemImpl,
}

impl InfoType {
	pub fn new(perf: &PerformanceDeclaration) -> InfoType {
		let payload_name = perf.payload_name();
		let info_name = perf.info_name();

		InfoType {
			definition: parse_quote! {
				struct #info_name {}
			},
			impl_block: parse_quote! {
				impl troupe::RoleInfo for #info_name {
					type Payload = #payload_name;
					type Sender = ::tokio::sync::mpsc::UnboundedSender<#payload_name>;
					type Receiver = ::tokio::sync::mpsc::UnboundedReceiver<#payload_name>;
				}
			},
		}
	}
}

impl ToTokens for InfoType {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.definition.to_tokens(tokens);
		self.impl_block.to_tokens(tokens);
	}
}
