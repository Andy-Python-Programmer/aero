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
use proc_macro2::{Ident, Span};

use quote::quote;

use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, FnArg, NestedMeta, Pat, Type};

enum ArgType {
    Array(bool),     // mutable?
    Slice(bool),     // mutable?
    Pointer(bool),   // mutable?
    Reference(bool), // mutable?
    String,
    Path,
}

#[derive(Default)]
struct Config {
    no_return: bool,
}

// TODO: determine if no_return by checking the return type of the syscall function;
// if its the never type <!>.
fn parse_attribute_args(args: Vec<NestedMeta>) -> Config {
    let mut config = Config::default();

    for attribute in args {
        match attribute {
            NestedMeta::Meta(meta) => match meta {
                syn::Meta::Path(path) => {
                    if let Some(ident) = path.get_ident() {
                        match ident.to_string().as_str() {
                            "no_return" => config.no_return = true,
                            _ => emit_error!(ident.span(), "unknown attribute"),
                        }
                    }
                }
                _ => emit_error!(meta.span(), "unknown attribute"),
            },
            NestedMeta::Lit(e) => emit_error!(e.span(), "unknown attribute"),
        }
    }

    config
}

pub fn parse(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as syn::AttributeArgs);
    let config = parse_attribute_args(attr);

    let parsed_fn = syn::parse_macro_input!(item as syn::ItemFn);
    let signature = &parsed_fn.sig;

    signature
        .constness
        .map(|e| emit_error!(e.span(), "syscall functions cannot be `const`"));

    signature
        .asyncness
        .map(|e| emit_error!(e.span(), "syscall functions cannot be `async`"));

    signature
        .unsafety
        .map(|e| emit_error!(e.span(), "syscalls functions cannot be `unsafe`"));

    let generics = &signature.generics;

    // NOTE: if `lt_token` is present then `gt_token` will also be present (else invalid syntax).
    if generics.lt_token.is_some() {
        let lt_span = generics.lt_token.span().unwrap();
        let gt_span = generics.gt_token.span().unwrap();

        let span_range = lt_span.join(gt_span).unwrap();

        emit_error!(
            span_range,
            "syscall functions cannot have generic parameters"
        );
    }

    let attrs = &parsed_fn.attrs;
    let vis = &parsed_fn.vis;
    let name = &signature.ident;
    let orig_args = &signature.inputs;
    let processed_args = process_args(orig_args);
    let call_args = process_call_args(orig_args);
    let args = orig_args
        .iter()
        .map(|e| match e {
            FnArg::Typed(arg) => match arg.pat.as_ref() {
                Pat::Ident(pat) => pat.ident.clone(),
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        })
        .collect::<Vec<_>>();
    let ret = &signature.output;
    let body = &parsed_fn.block;

    let syslog_args = orig_args
        .iter()
        .map(|e| match e {
            FnArg::Typed(typed) => match typed.pat.as_ref() {
                Pat::Ident(pat) => {
                    let ident = &pat.ident;
                    let typ = determine_arg_type(&typed.ty);

                    if let Some(typ) = typ {
                        match typ {
                            ArgType::Array(_) => quote::quote!(.add_argument("<array>")),
                            ArgType::Slice(_) => quote::quote!(.add_argument(alloc::format!("<slice[..{}]>", #ident.len()))),

                            ArgType::Pointer(_) => {
                                quote::quote!(.add_argument(alloc::format!("*{:#x}", #ident as usize)))
                            }

                            ArgType::Reference(_) => {
                                quote::quote!(.add_argument(alloc::format!("&{:#x}", #ident as *const _ as usize)))
                            },

                            ArgType::String | ArgType::Path => quote::quote!(.add_argument_dbg(#ident)), 
                        }
                    } else {
                        quote::quote!(.add_argument(#ident))
                    }
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();

    let syslog = quote::quote! {
        #[cfg(feature = "syslog")]
        crate::syscall::SysLog::new(stringify!(#name))
            #(#syslog_args)*
            .set_result(result)
            .flush();
    };

    let compiled_body = if config.no_return {
        quote::quote! {
            fn even_inner(#orig_args) #ret #body
            even_inner(#(#call_args),*)
        }
    } else {
        quote::quote! {
            #[inline(always)]
            fn even_inner(#orig_args) #ret #body

            #[inline(always)]
            fn syscall_inner(#orig_args) #ret {
                let result = even_inner(#(#args),*);
                #syslog
                result
            }

            syscall_inner(#(#call_args),*)
        }
    };

    let result = quote! {
        #(#attrs)* #vis fn #name(#(#processed_args),*) #ret {
            #compiled_body
        }
    };

    result.into()
}

fn determine_arg_type(typ: &Type) -> Option<ArgType> {
    match typ {
        Type::Reference(typ) => match typ.elem.as_ref() {
            Type::Array(_) => Some(ArgType::Array(typ.mutability.is_some())),
            Type::Slice(_) => Some(ArgType::Slice(typ.mutability.is_some())),
            Type::Path(path) => {
                if path.path.segments.last().unwrap().ident == "str" {
                    Some(ArgType::String)
                } else if path.path.segments.last().unwrap().ident == "Path" {
                    // NOTE: This will match to any type that has the name "Path"
                    Some(ArgType::Path)
                } else {
                    Some(ArgType::Reference(typ.mutability.is_some()))
                }
            }
            _ => None,
        },
        Type::Ptr(typ) => Some(ArgType::Pointer(typ.mutability.is_some())),
        _ => None,
    }
}

fn process_args(args: &Punctuated<FnArg, syn::Token![,]>) -> Vec<FnArg> {
    let mut result = Vec::new();

    for arg in args {
        match arg {
            FnArg::Typed(arg) => match arg.pat.as_ref() {
                Pat::Ident(pat) => {
                    let attrs = &arg.attrs;
                    let typ = &arg.ty;
                    let ident = &pat.ident;

                    match determine_arg_type(typ) {
                        Some(ArgType::Slice(_) | ArgType::String | ArgType::Path) => {
                            let data = Ident::new(&format!("{}_data", ident), Span::call_site());
                            let len = Ident::new(&format!("{}_len", ident), Span::call_site());

                            result.push(syn::parse_quote!(#data: usize));
                            result.push(syn::parse_quote!(#len: usize));
                        }
                        Some(ArgType::Array(_)) => {
                            let data = Ident::new(&format!("{}_data", ident), Span::call_site());

                            result.push(syn::parse_quote!(#data: usize));
                        }
                        Some(ArgType::Pointer(_)) | Some(ArgType::Reference(_)) => {
                            result.push(syn::parse_quote!(#(#attrs)* #ident: usize));
                        }
                        None => {
                            result.push(syn::parse_quote!(#(#attrs)* #ident: #typ));
                        }
                    };
                }
                _ => {
                    emit_error!(
                        arg.span(),
                        "syscall function arguments cannot have non-ident patterns"
                    );
                }
            },
            FnArg::Receiver(_) => {
                emit_error!(
                    arg.span(),
                    "syscall functions cannot have receiver arguments"
                );
            }
        }
    }

    result
}

fn process_call_args(args: &Punctuated<FnArg, syn::Token![,]>) -> Vec<Expr> {
    let mut result = Vec::new();

    for arg in args {
        match arg {
            FnArg::Typed(arg) => match arg.pat.as_ref() {
                Pat::Ident(pat) => {
                    let ty = &arg.ty;
                    let ident = &pat.ident;

                    if let Some(arg_type) = determine_arg_type(ty) {
                        let data_ident = Ident::new(&format!("{}_data", ident), Span::call_site());
                        let len_ident = Ident::new(&format!("{}_len", ident), Span::call_site());

                        match arg_type {
                            ArgType::Slice(is_mut) => {
                                let slice_expr: Expr = if is_mut {
                                    syn::parse_quote! {
                                        crate::utils::validate_slice_mut(#data_ident as *mut _, #len_ident).ok_or(SyscallError::EINVAL)?
                                    }
                                } else {
                                    syn::parse_quote! {
                                        crate::utils::validate_slice(#data_ident as *const _, #len_ident).ok_or(SyscallError::EINVAL)?
                                    }
                                };

                                result.push(slice_expr);
                            }
                            ArgType::Array(is_mut) => {
                                let array_expr: Expr = if is_mut {
                                    syn::parse_quote! {
                                        crate::utils::validate_array_mut(#data_ident as *mut _).ok_or(SyscallError::EINVAL)?
                                    }
                                } else {
                                    unimplemented!()
                                };

                                result.push(array_expr);
                            }
                            ArgType::Pointer(is_mut) => {
                                let ptr_expr: Expr = if is_mut {
                                    syn::parse_quote!(#ident as *mut _)
                                } else {
                                    syn::parse_quote!(#ident as *const _)
                                };

                                result.push(ptr_expr);
                            }
                            ArgType::Reference(is_mut) => {
                                let ref_expr: Expr = if is_mut {
                                    syn::parse_quote!({
                                        crate::utils::validate_mut_ptr(#ident as *mut _).ok_or(SyscallError::EINVAL)?
                                    })
                                } else {
                                    syn::parse_quote!({
                                        crate::utils::validate_ptr(#ident as *const _).ok_or(SyscallError::EINVAL)?
                                    })
                                };

                                result.push(ref_expr);
                            }
                            ArgType::String => result.push(syn::parse_quote! {
                                crate::utils::validate_str(#data_ident as *const _, #len_ident).ok_or(SyscallError::EINVAL)?
                            }),
                            ArgType::Path => result.push(syn::parse_quote! {
                                {
                                    let string = crate::utils::validate_str(#data_ident as *const _, #len_ident).ok_or(SyscallError::EINVAL)?;
                                    let path = Path::new(string);
                                    path
                                }
                            }),
                        }
                    } else {
                        result.push(syn::parse_quote!(#ident));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    result
}
