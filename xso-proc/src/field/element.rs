// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module concerns the processing of untyped `minidom::Element`
//! children.
//!
//! In particular, it provides the `#[xml(element)]` implementation.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::*;

use crate::error_message::{self, ParentRef};
use crate::meta::{AmountConstraint, Flag};
use crate::scope::{AsItemsScope, FromEventsScope};
use crate::types::{
    as_xml_iter_fn, default_fn, element_ty, from_events_fn, from_xml_builder_ty,
    into_iterator_into_iter_fn, into_iterator_item_ty, into_iterator_iter_ty, item_iter_ty,
    option_ty, ref_ty,
};

use super::{Field, FieldBuilderPart, FieldIteratorPart, FieldTempInit, NestedMatcher};

pub(super) struct ElementField {
    /// Flag indicating whether the value should be defaulted if the
    /// child is absent.
    pub(super) default_: Flag,

    /// Number of child elements allowed.
    pub(super) amount: AmountConstraint,
}

impl Field for ElementField {
    fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldBuilderPart> {
        let element_ty = match self.amount {
            AmountConstraint::FixedSingle(_) => ty.clone(),
            AmountConstraint::Any(_) => into_iterator_item_ty(ty.clone()),
        };

        let FromEventsScope {
            ref substate_result,
            ..
        } = scope;

        let from_events = from_events_fn(element_ty.clone());

        let extra_defs = TokenStream::default();
        let field_access = scope.access_field(member);

        let default_fn = default_fn(ty.clone());
        let builder = from_xml_builder_ty(element_ty.clone());

        match self.amount {
            AmountConstraint::FixedSingle(_) => {
                let missing_msg = error_message::on_missing_child(container_name, member);
                let on_absent = match self.default_ {
                    Flag::Absent => quote! {
                        return ::core::result::Result::Err(::xso::error::Error::Other(#missing_msg).into())
                    },
                    Flag::Present(_) => {
                        quote! { #default_fn() }
                    }
                };
                Ok(FieldBuilderPart::Nested {
                    extra_defs,
                    value: FieldTempInit {
                        init: quote! { ::core::option::Option::None },
                        ty: option_ty(ty.clone()),
                    },
                    matcher: NestedMatcher::Selective(quote! {
                        if #field_access.is_some() {
                            ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch { name, attrs })
                        } else {
                            #from_events(name, attrs, ctx)
                        }
                    }),
                    builder,
                    collect: quote! {
                        #field_access = ::core::option::Option::Some(#substate_result);
                    },
                    finalize: quote! {
                        match #field_access {
                            ::core::option::Option::Some(value) => value,
                            ::core::option::Option::None => #on_absent,
                        }
                    },
                })
            }
            AmountConstraint::Any(_) => Ok(FieldBuilderPart::Nested {
                extra_defs,
                value: FieldTempInit {
                    init: quote! { #default_fn() },
                    ty: ty.clone(),
                },
                matcher: NestedMatcher::Fallback(quote! {
                    #builder::new(name, attrs)
                }),
                builder,
                collect: quote! {
                    <#ty as ::core::iter::Extend::<#element_ty>>::extend(&mut #field_access, [#substate_result]);
                },
                finalize: quote! {
                    #field_access
                },
            }),
        }
    }

    fn make_iterator_part(
        &self,
        scope: &AsItemsScope,
        _container_name: &ParentRef,
        bound_name: &Ident,
        _member: &Member,
        ty: &Type,
    ) -> Result<FieldIteratorPart> {
        let AsItemsScope { ref lifetime, .. } = scope;

        let item_ty = match self.amount {
            AmountConstraint::FixedSingle(_) => ty.clone(),
            AmountConstraint::Any(_) => {
                // This should give us the type of element stored in the
                // collection.
                into_iterator_item_ty(ty.clone())
            }
        };

        let element_ty = element_ty(Span::call_site());
        let iter_ty = item_iter_ty(element_ty.clone(), lifetime.clone());
        let element_iter = into_iterator_iter_ty(ref_ty(ty.clone(), lifetime.clone()));
        let into_iter = into_iterator_into_iter_fn(ref_ty(ty.clone(), lifetime.clone()));

        let state_ty = Type::Tuple(TypeTuple {
            paren_token: token::Paren::default(),
            elems: [element_iter, option_ty(iter_ty.clone())]
                .into_iter()
                .collect(),
        });

        let extra_defs = TokenStream::default();
        let as_xml_iter = as_xml_iter_fn(item_ty.clone());
        let init = quote! { #as_xml_iter(#bound_name)? };
        let iter_ty = item_iter_ty(item_ty.clone(), lifetime.clone());

        match self.amount {
            AmountConstraint::FixedSingle(_) => Ok(FieldIteratorPart::Content {
                extra_defs,
                value: FieldTempInit { init, ty: iter_ty },
                generator: quote! {
                    #bound_name.next().transpose()
                },
            }),
            AmountConstraint::Any(_) => Ok(FieldIteratorPart::Content {
                extra_defs,
                value: FieldTempInit {
                    init: quote! {
                        (#into_iter(#bound_name), ::core::option::Option::None)
                    },
                    ty: state_ty,
                },
                generator: quote! {
                    loop {
                        if let ::core::option::Option::Some(current) = #bound_name.1.as_mut() {
                            if let ::core::option::Option::Some(item) = current.next() {
                                break ::core::option::Option::Some(item).transpose();
                            }
                        }
                        if let ::core::option::Option::Some(item) = #bound_name.0.next() {
                            #bound_name.1 = ::core::option::Option::Some(
                                <#element_ty as ::xso::AsXml>::as_xml_iter(item)?
                            );
                        } else {
                            break ::core::result::Result::Ok(::core::option::Option::None)
                        }
                    }
                },
            }),
        }
    }
}
