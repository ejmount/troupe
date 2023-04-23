use convert_case::{Case, Casing};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use std::fmt::Debug;
use syn::{parse::Parser, parse_quote, Field, Item, ItemImpl, ItemStruct, Path};

#[derive(Clone)]
pub struct Role {
    pub typ: Path,
    pub name: Ident,
    pub field_name: Ident,
    pub info_name: Ident,
}

impl Role {
    pub fn new(typ: Path) -> Role {
        let name = typ
            .segments
            .last()
            .expect("Tried to make Role out of empty path - this is a bug, please file an issue");

        let name = name.ident.to_string().to_case(Case::UpperCamel);
        let field_name = format_ident!("{}", name.to_case(Case::Snake));
        let info_name = format_ident!("{}Info", name.to_case(Case::Snake));
        let name = format_ident!("{name}");

        Role {
            name,
            typ,
            field_name,
            info_name,
        }
    }
}

impl Debug for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Role")
            .field("name", &self.name.to_string())
            .finish()
    }
}

pub fn create_actor_type(roles: &[Role], actor_name: &Ident) -> ItemStruct {
    let fields = roles.iter().map(
        |Role {
             field_name, name, ..
         }|
         -> Field {
            Field::parse_named
                .parse2(quote! {#field_name : <<Self as #name>::Info as troupe::RoleInfo>::Sender})
                .unwrap()
        },
    );
    parse_quote! {
        struct #actor_name {
            #(#fields),*
        }
    }
}

pub fn create_actor_impls(roles: &[Role], actor_name: &Ident) -> Vec<ItemImpl> {
    fn create_impl(
        Role {
            typ,
            field_name,
            name,
            info_name,
            ..
        }: &Role,
        actor_name: &Ident,
    ) -> ItemImpl {
        parse_quote! {
            //#[::async_trait::async_trait]
            impl #typ for #actor_name {
                type Info = #info_name;
                fn send(&self, msg: impl Into<<<Self as #name>::Info as troupe::RoleInfo>::Payload> + Send) -> Result<(), <<<Self as #name>::Info as troupe::RoleInfo>::Sender as troupe::RoleSender>::Error>
                {
                    self.#field_name.send(msg.into())
                }
            }
        }
    }
    roles.iter().map(|r| create_impl(r, actor_name)).collect()
}

fn make_payload_name(r: &Role) -> Ident {
    format_ident!("{}Payload", r.name.to_string().to_case(Case::UpperCamel))
}

pub fn create_role(
    role @ Role {
        name, info_name, ..
    }: &Role,
) -> Vec<Item> {
    let payload_name = make_payload_name(role);
    let trait_ = parse_quote! {
        //#[::async_trait::async_trait]
        trait #name: Sized {
            type Info: troupe::RoleInfo;
            fn send(&self, msg: impl Into<#payload_name>+Send) -> Result<(), ::tokio::sync::mpsc::error::SendError<#payload_name>>;
        }
    };
    let info_struct = parse_quote! {
        struct #info_name {}
    };
    let impl_ = parse_quote! {
        impl troupe::RoleInfo for #info_name {
            type Payload = #payload_name;
            type Sender = ::tokio::sync::mpsc::UnboundedSender<#payload_name>;
            type Receiver = ::tokio::sync::mpsc::UnboundedReceiver<#payload_name>;
        }
    };
    vec![trait_, info_struct, impl_]
}

pub fn create_payload(r: &Role) -> ItemStruct {
    let ident = make_payload_name(r);
    parse_quote! {
        pub struct #ident {}
    }
}
