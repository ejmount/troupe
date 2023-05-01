use quote::format_ident;
use syn::fold::Fold;
use syn::{parse_quote, Block, Pat, Path, Receiver, ReturnType};

pub struct NameRewriter {
	index:     usize,
	role_name: Path,
}
impl NameRewriter {
	pub fn new(role_name: Path) -> NameRewriter {
		NameRewriter {
			index: 0,
			role_name,
		}
	}
}

impl Fold for NameRewriter {
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
		parse_quote! {-> Result <(), <<dyn #role_name as troupe::Role>::Sender as troupe::RoleSender>:: Error >}
	}

	fn fold_block(&mut self, i: Block) -> Block {
		i // Don't recurse
	}
}
