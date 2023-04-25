use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use syn::{
    parse::Parser, parse_quote, Arm, Error, Field, FnArg, ImplItem, ImplItemFn, Item, ItemEnum,
    ItemFn, ItemImpl, ItemStruct, Stmt, Variant,
};

use crate::types::{Performance, Role};

macro_rules! filter_unwrap {
    ($list:expr, $pat:path) => {
        $list
            .iter()
            .cloned()
            .filter_map(|item| if let $pat(a) = item { Some(a) } else { None })
    };
}

macro_rules! bail_if_any {
    ($list:ident) => {
        if let Some(err) = $list.into_iter().reduce(|mut e, f| -> syn::Error {
            e.combine(f);
            e
        }) {
            return Err(err);
        }
    };
}

pub fn create_actor_type(roles: &[Role], actor_name: &Ident) -> ItemStruct {
    fn create_field(role: &Role) -> Field {
        let Role {
            field_name, name, ..
        } = role;
        Field::parse_named
            .parse2(quote! {#field_name : <<Self as #name>::Info as troupe::RoleInfo>::Sender})
            .unwrap_or_else(|_| panic!("Parse failure trying to create actor field {name} - this is a bug, please file an issue"))
    }
    let fields = roles.iter().map(create_field);
    parse_quote! {
        struct #actor_name {
            #(#fields),*
        }
    }
}

pub fn create_actor_impls(roles: &[Role], actor_name: &Ident) -> Vec<ItemImpl> {
    fn create_impl(role: &Role, actor_name: &Ident) -> ItemImpl {
        let Role {
            typ,
            field_name,
            name,
            info_name,
            ..
        } = role;
        parse_quote! {
            #[::async_trait::async_trait]
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

pub fn create_role(role: &Role) -> Vec<Item> {
    let Role {
        name, info_name, ..
    } = role;
    let payload_name = make_payload_name(role);
    let trait_ = parse_quote! {
        #[::async_trait::async_trait]
        trait #name {
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

pub fn create_payload(r: &Role, p: &Performance) -> ItemEnum {
    fn make_variant(function: &ImplItemFn) -> Variant {
        let variant_name = make_variant_name(function);

        let types = filter_unwrap!(function.sig.inputs, FnArg::Typed).map(|pat| *pat.ty);
        parse_quote! { #variant_name ((#(#types),*)) }
    }

    let payload_name = make_payload_name(r);

    let variants = p.handlers.iter().map(make_variant);

    parse_quote! {
        pub enum #payload_name { #(#variants),* }
    }
}

fn make_variant_name(function: &ImplItemFn) -> Ident {
    let name = function.sig.ident.to_string();
    format_ident!("{}", name.to_case(Case::UpperCamel))
}
pub fn create_performance(imp: ItemImpl) -> Result<Performance, Error> {
    let Some((_, role_path, _)) = &imp.trait_ else { return Err(Error::new_spanned(imp, "Must impl a role trait")) };

    let fns = filter_unwrap!(imp.items, ImplItem::Fn).collect_vec();

    let invalid_funs = fns
        .iter()
        .filter(|fun| {
            if let Some(FnArg::Receiver(recv)) = fun.sig.inputs.first() {
                recv.mutability.is_none() || recv.reference.is_none()
            } else {
                true
            }
        })
        .map(|fun| Error::new_spanned(fun, "Handler must take self by mut reference"));

    bail_if_any!(invalid_funs);

    Ok(Performance::new(fns, role_path))
}

pub fn perform_fn_name(role_name: &Ident) -> Ident {
    format_ident!("{}", format!("perform_{role_name}").to_case(Case::Snake))
}

pub fn create_handling_block(name: &Ident, roles: &[(Role, Performance)]) -> ItemImpl {
    let handlers = roles.iter().map(|(r, p)| create_handler(r, p));

    parse_quote! {
        impl #name {
            #(#handlers)*
        }
    }
}

pub fn create_handler(role: &Role, performance: &Performance) -> ItemFn {
    let payload_name = make_payload_name(role);
    let arms = performance.handlers.iter().map(|fun| -> Arm {
        let patterns = filter_unwrap!(fun.sig.inputs, FnArg::Typed);

        let variant_name = make_variant_name(fun);

        let body = &fun.block;
        parse_quote! {
            #payload_name::#variant_name ((#(#patterns),*)) => #body,
        }
    });

    let handler_name = &performance.fn_name;
    parse_quote! {
        fn #handler_name(&mut self, msg: #payload_name) {
            match msg {
                #(#arms),*
            }
        }
    }
}

pub fn create_spawner(data_name: &Ident, actor_name: &Ident, roles: &[Role]) -> ItemImpl {
    let input_field_names = roles
        .iter()
        .map(|r| format_ident!("{}_input", r.field_name))
        .collect_vec();

    let output_field_names = roles
        .iter()
        .map(|r| format_ident!("{}_output", r.field_name))
        .collect_vec();

    let queue_constructions = itertools::izip!(roles, &input_field_names, &output_field_names).map(
        |(role, inn, out)| -> Stmt {
            let constructor = &role.constructor_expr;
            parse_quote! { let (#inn, mut #out) = #constructor; }
        },
    );

    let actor_fields = itertools::izip!(roles, &input_field_names)
        .map(|(r, input)| -> Field {
            let field_name = &r.field_name;
            Field::parse_named
            .parse2(quote! {#field_name : #input})
            .unwrap_or_else(|_| panic!("Parse failure trying to create actor field {data_name} - this is a bug, please file an issue"))
        });

    let select_branches =
        itertools::izip!(roles, output_field_names.iter()).map(|(role, output)| -> TokenStream {
            let fn_name = perform_fn_name(&role.name);
            quote! { Some(msg) = #output.recv() => {
                state.#fn_name(msg);
            } }
        });

    parse_quote! {
        impl #data_name {
            pub fn start(mut state: #data_name) -> (::std::sync::Arc<#actor_name>, ::tokio::task::JoinHandle<()>) {
                #(#queue_constructions)*
                let actor = #actor_name {
                    #(#actor_fields),*
                };
                let actor_handle = ::std::sync::Arc::new(actor);
                let event_loop = async move {
                    loop {
                        ::tokio::select! {
                            #(#select_branches),*
                            else => break
                        }
                    }
                };
                let join_handle = ::tokio::task::spawn(event_loop);
                (actor_handle, join_handle)
            }
        }
    }
}
