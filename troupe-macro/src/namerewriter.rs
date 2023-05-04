use quote::format_ident;
use syn::fold::Fold;
use syn::{parse_quote, Block, Pat, Path, Receiver, ReturnType};

pub struct MethodRewriter {
	index:     usize,
	role_name: Path,
}
impl MethodRewriter {
	pub fn new(role_name: Path) -> MethodRewriter {
		MethodRewriter {
			index: 0,
			role_name,
		}
	}
}

impl Fold for MethodRewriter {
	fn fold_pat(&mut self, _: Pat) -> Pat {
		let ident = format_ident!("_{}", self.index);
		self.index += 1;
		parse_quote! { #ident }
	}

	fn fold_receiver(&mut self, _: Receiver) -> Receiver {
		parse_quote! { &self }
	}

	fn fold_return_type(&mut self, _: ReturnType) -> ReturnType {
		let role_name = &self.role_name;
		parse_quote! {-> Result <(), <<<dyn #role_name as troupe::Role>::Channel as troupe::Channel>::Sender as troupe::RoleSender<<dyn #role_name as troupe::Role>::Payload>>::Error >}
	}

	fn fold_signature(&mut self, i: syn::Signature) -> syn::Signature {
		// Visit deeper
		let sig = syn::fold::fold_signature(self, i);

		if sig.asyncness.is_none() {
			parse_quote! { async #sig }
		} else {
			sig
		}
	}

	fn fold_block(&mut self, i: Block) -> Block {
		i // Don't recurse because we don't want to modify the contents
	}
}
