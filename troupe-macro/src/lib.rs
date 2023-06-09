#![warn(clippy::pedantic)]

mod actor_decl;
mod attributes;
mod macros;
mod namerewriter;
mod performance;
mod role;
mod spawning_function;

use actor_decl::ActorDecl;
use performance::Performance;
use proc_macro::TokenStream as TokenStream1;
use quote::ToTokens;
use syn::{Error, ItemMod};

fn actor_core_new(module: ItemMod) -> Result<ActorDecl, Error> {
	let Some((_, items)) = module.content else { return Err(Error::new_spanned(module, "Module must be non-empty")) };

	ActorDecl::new(&items)
}

///
///
/// # Panics
/// Don't be malformed.
#[proc_macro_attribute]
pub fn actor(_attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
	let module = syn::parse_macro_input!(item as ItemMod);

	match actor_core_new(module) {
		Ok(actor_decl) => actor_decl.to_token_stream().into(),
		Err(e) => e.into_compile_error().into_token_stream().into(),
	}
}
