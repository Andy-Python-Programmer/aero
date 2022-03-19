/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{ItemImpl, ItemTrait, Result, Token, Type};

struct ObjectImpl {
    pub name: Box<Type>,
    pub _as_token: Token![as],
    pub ty: Box<Type>,
}
impl Parse for ObjectImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(ObjectImpl {
            name: input.parse()?,
            _as_token: input.parse()?,
            ty: input.parse()?,
        })
    }
}

#[proc_macro_attribute]
pub fn def(attr: TokenStream, input: TokenStream) -> TokenStream {
    let input: ItemTrait = syn::parse_macro_input!(input as ItemTrait);
    let attr: syn::LitStr = syn::parse_macro_input!(attr as syn::LitStr);

    let name = input.ident;

    // async fn foo(&self);
    // async fn create() -> TestObj;

    let methods = input
        .items
        .into_iter()
        .filter_map(|a| match a {
            syn::TraitItem::Method(m) => Some(m),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut methods2 = vec![];
    for method in methods.into_iter() {
        let inputs = method.sig.inputs.clone();
        let recievers = inputs
            .iter()
            .filter_map(|a| match a {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(r) => Some(r.ty.clone()),
            })
            .collect::<Vec<_>>();

        let mut args2 = vec![];
        let mut serialize_args2 = vec![];
        let mut i = -1;
        for recv in recievers {
            i += 1;
            let ident = proc_macro2::Ident::new(&format!("arg_{}", i), proc_macro2::Span::call_site());
            args2.push(quote::quote! {
                #ident: #recv
            });
            serialize_args2.push(quote::quote! {
                #ident
            });
        }

        let func_name = method.sig.ident;
        let func_name_str = func_name.to_string();
        let ret = method.sig.output;
        let output = if inputs.len() == 0
            || match &inputs[0] {
                syn::FnArg::Receiver(_) => false,
                syn::FnArg::Typed(_) => true,
            } {
            // static method
            quote::quote! {
                // TODO: i (pitust) am not happy abut these unwraps
                pub async fn #func_name(target_pid: usize, #(#args2),*) #ret {
                    let (id, fut) = ::aipc::async_runtime::alloc_reply_id();
                    let req = ::aipc::serialize_buffer((
                        id - 1,
                        #func_name_str,
                        #attr,
                        (#(#serialize_args2,)*)
                    )).unwrap();
                    ::aipc::__private::sys_ipc_send(target_pid, &req).unwrap();
                    Self(::aipc::ClientObject {
                        pid: target_pid,
                        object_id: ::aipc::deserialize_object(&fut.await.message[8..]).unwrap()
                    })
                }
            }
        } else {
            // instance method
            quote::quote! {
                pub async fn #func_name(&self, #(#args2),*) #ret {
                    let (id, fut) = ::aipc::async_runtime::alloc_reply_id();
                    let req = ::aipc::serialize_buffer((
                        id - 1,
                        #func_name_str,
                        #attr,
                        self.0.object_id,
                        (#(#serialize_args2,)*)
                    )).unwrap();
                    ::aipc::__private::sys_ipc_send(self.0.pid, &req).unwrap();
                    ::aipc::deserialize_object(&fut.await.message[8..]).unwrap()
                }
            }
        };
        methods2.push(output);
    }

    quote::quote! {
        pub struct #name(::aipc::ClientObject);
        impl #name {
            pub const __OBJECT_PATH: &'static str = #attr;
            #(#methods2)*
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn object(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as ObjectImpl);
    let the_impl = syn::parse_macro_input!(input as ItemImpl);
    let target = attr.ty;

    let mut fragments = vec![];

    let name = attr.name;
    let data = the_impl.self_ty.clone();
    let t_items = the_impl.items;

    for input in t_items.clone() {
        let input = match input {
            syn::ImplItem::Method(m) => m,
            _ => return quote::quote! { compile_error!("cannot define object") }.into(),
        };

        let is_async = input.sig.asyncness.is_some();
        let tgd_nam = input.sig.ident.clone();
        let mystr = tgd_nam.to_string();
        let args: Vec<_> = input
            .sig
            .inputs
            .iter()
            .filter_map(|a| match a {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(t) => Some(t.ty.clone()),
            })
            .map(|a| {
                quote::quote! {
                    ::aipc::deserialize::<#a>(&mut request_deserializer).unwrap()
                }
            })
            .collect();
        let possibly_await = if is_async {
            quote::quote! {.await }
        } else {
            quote::quote! {}
        };
        if input.sig.inputs.len() == 0
            || if let syn::FnArg::Receiver(_) = &input.sig.inputs[0] {
                false
            } else {
                true
            }
        {
            // this method does not have a reciever, it *must* be a factory
            fragments.push(quote::quote! {
                #mystr => {
                    let d: #data = #data::#tgd_nam (
                        #(
                            #args
                        ),*
                    ) #possibly_await;
                    ::aipc::serialize_buffer(self.0.create(source, d))
                },
            });
        } else {
            // method has a reciever
            fragments.push(quote::quote! {
                #mystr => {
                    let obj = self.0
                        .get(source, ::aipc::deserialize::<usize>(&mut request_deserializer)?)?;
                    let mut obj = obj
                        .lock();

                    ::aipc::serialize_buffer(obj.#tgd_nam (
                        #(
                            #args
                        ),*
                    ) #possibly_await )
                },
            });
        }
    }

    let result = quote::quote! {
        pub struct #name(::aipc::ServerObject<#data>);
        impl #data {
            #(#t_items)*
        }
        impl ::aipc::async_runtime::Listener for #name {
            fn listen() {
                let srv = ::aipc::__private::Arc::new(::aipc::__private::Mutex::new(Self::create_server()));
                aipc::async_runtime::create_server(Box::new(move |msg| {
                    let srv = ::aipc::__private::Arc::clone(&srv);
                    aipc::async_runtime::spawn(async move {
                        let mut srv = srv.lock();
                        match srv.service_request(msg.pid, &msg.message[8..]).await {
                            Some(data) => {
                                let data: ::aipc::__private::Vec<u8> = data; // type annotation for rust-analyzer

                                let mut data = [&msg.message[0..8], &data].concat();
                                data[0] += 1; // set the reply bit
                                ::aero_syscall::sys_ipc_send(msg.pid, &data).unwrap();
                            }
                            None => {}
                        }
                    });

                    // TODO: we should be thruthful here
                    true
                }));
            }
        }
        impl #name {
            fn create_server() -> #name {
                #name(::aipc::ServerObject::<#data>::new())
            }
            async fn service_request(&mut self, source: usize, data: &[u8]) -> Option<Vec<u8>> {
                extern crate alloc;
                let mut request_deserializer = ::aipc::deserializer(data);
                let typ = ::aipc::deserialize::<alloc::string::String>(&mut request_deserializer)?;
                let call_target = ::aipc::deserialize::<alloc::string::String>(&mut request_deserializer)?;
                if #target::__OBJECT_PATH != call_target {
                    return None
                }
                match typ.as_str() {
                    #(#fragments)*
                    "__drop" => {
                        // drop the object
                        self.0.do_drop(
                            source,
                            ::aipc::deserialize::<usize>(&mut request_deserializer)?
                        );
                        ::aipc::serialize_buffer(())
                    },
                    _ => {
                        println!("[aipc] call to unhandled function {}.{}", call_target, typ);
                        None
                    }
                }
            }
        }
    };

    result.into()
}
