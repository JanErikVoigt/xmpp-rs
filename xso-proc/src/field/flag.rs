// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module concerns the processing of flag-style children.
//!
//! In particular, it provides the `#[xml(flag)]` implementation.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::*;

use crate::error_message::{FieldName, ParentRef};
use crate::meta::{NameRef, NamespaceRef};
use crate::scope::{AsItemsScope, FromEventsScope};
use crate::types::{bool_ty, empty_builder_ty, u8_ty};

use super::{Field, FieldBuilderPart, FieldIteratorPart, FieldTempInit, NestedMatcher};

/// The field maps to a child element, the presence of which is represented as boolean.
pub(super) struct FlagField {
    /// The XML namespace of the child element.
    pub(super) xml_namespace: NamespaceRef,

    /// The XML name of the child element.
    pub(super) xml_name: NameRef,
}

impl Field for FlagField {
    fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
        member: &Member,
        _ty: &Type,
    ) -> Result<FieldBuilderPart> {
        let field_access = scope.access_field(member);

        let unknown_attr_err = format!(
            "Unknown attribute in flag child {} in {}.",
            FieldName(member),
            container_name
        );
        let unknown_child_err = format!(
            "Unknown child in flag child {} in {}.",
            FieldName(member),
            container_name
        );
        let unknown_text_err = format!(
            "Unexpected text in flag child {} in {}.",
            FieldName(member),
            container_name
        );

        let xml_namespace = &self.xml_namespace;
        let xml_name = &self.xml_name;

        Ok(FieldBuilderPart::Nested {
            extra_defs: TokenStream::new(),
            value: FieldTempInit {
                ty: bool_ty(Span::call_site()),
                init: quote! { false },
            },
            matcher: NestedMatcher::Selective(quote! {
                if name.0 == #xml_namespace && name.1 == #xml_name {
                    ::xso::fromxml::Empty {
                        attributeerr: #unknown_attr_err,
                        childerr: #unknown_child_err,
                        texterr: #unknown_text_err,
                    }.start(attrs).map_err(
                        ::xso::error::FromEventsError::Invalid
                    )
                } else {
                    ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch {
                        name,
                        attrs,
                    })
                }
            }),
            builder: empty_builder_ty(Span::call_site()),
            collect: quote! {
                #field_access = true;
            },
            finalize: quote! {
                #field_access
            },
        })
    }

    fn make_iterator_part(
        &self,
        _scope: &AsItemsScope,
        _container_name: &ParentRef,
        bound_name: &Ident,
        _member: &Member,
        _ty: &Type,
    ) -> Result<FieldIteratorPart> {
        let xml_namespace = &self.xml_namespace;
        let xml_name = &self.xml_name;

        Ok(FieldIteratorPart::Content {
            extra_defs: TokenStream::new(),
            value: FieldTempInit {
                init: quote! {
                    if *#bound_name {
                        3
                    } else {
                        1
                    }
                },
                ty: u8_ty(Span::call_site()),
            },
            generator: quote! {
                {
                    // using wrapping_sub will make the match below crash
                    // with unreachable!() in case we messed up somewhere.
                    #bound_name = #bound_name.wrapping_sub(1);
                    match #bound_name {
                        0 => ::core::result::Result::<_, ::xso::error::Error>::Ok(::core::option::Option::None),
                        1 => ::core::result::Result::Ok(::core::option::Option::Some(
                            ::xso::Item::ElementFoot
                        )),
                        2 => ::core::result::Result::Ok(::core::option::Option::Some(
                            ::xso::Item::ElementHeadStart(
                                ::xso::exports::rxml::Namespace::from(#xml_namespace),
                                ::xso::exports::alloc::borrow::Cow::Borrowed(#xml_name),
                            )
                        )),
                        _ => unreachable!(),
                    }
                }
            },
        })
    }
}
