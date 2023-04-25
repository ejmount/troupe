use convert_case::{Case, Casing};
use quote::format_ident;
use syn::parse_quote;
use syn::Expr;
use syn::Ident;
use syn::ImplItemFn;
use syn::Path;

#[derive(Clone)]
pub struct Role {
    pub name: Ident,
    pub typ: Path,
    pub field_name: Ident,
    pub info_name: Ident,
    pub constructor_expr: Expr,
}

impl Role {
    pub fn new(typ: Path) -> Role {
        let name = get_leaf_name(&typ).to_string().to_case(Case::UpperCamel);

        let info_name = format_ident!("{}Info", name.to_case(Case::UpperCamel));
        let field_name = format_ident!("{}", name.to_case(Case::Snake));
        let name = format_ident!("{name}");

        let constructor_expr = parse_quote! { ::tokio::sync::mpsc::unbounded_channel() };

        Role {
            name,
            typ,
            field_name,
            info_name,
            constructor_expr,
        }
    }
}

impl std::fmt::Debug for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Role")
            .field("name", &self.name.to_string())
            .finish()
    }
}

pub fn get_leaf_name(typ: &Path) -> &Ident {
    let name = typ
        .segments
        .last()
        .expect("Tried to make Role out of an empty path - this is a bug, please file an issue");
    &name.ident
}

pub struct Performance {
    pub fn_name: Ident,
    pub handlers: Vec<ImplItemFn>,
}

impl Performance {
    pub fn new(handlers: Vec<ImplItemFn>, role_path: &Path) -> Performance {
        let fn_name = crate::actor_decl::perform_fn_name(get_leaf_name(role_path));

        Performance { fn_name, handlers }
    }
}

impl std::fmt::Debug for Performance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Role")
            .field("fn_name", &self.fn_name.to_string())
            .finish()
    }
}
