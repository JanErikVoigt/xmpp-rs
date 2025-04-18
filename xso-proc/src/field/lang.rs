// Copyright (c) 2025 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module concerns the processing of inherited `xml:lang` values.
//!
//! In particular, it provides the `#[xml(lang)]` implementation.

use proc_macro2::Span;
use quote::quote;
use syn::*;

use crate::error_message::ParentRef;
use crate::scope::{AsItemsScope, FromEventsScope};
use crate::types::{as_optional_xml_text_fn, option_ty, string_ty};

use super::{Field, FieldBuilderPart, FieldIteratorPart, FieldTempInit};

/// The field maps to a potentially inherited `xml:lang` value.
pub(super) struct LangField;

impl Field for LangField {
    fn make_builder_part(
        &self,
        _scope: &FromEventsScope,
        _container_name: &ParentRef,
        _member: &Member,
        _ty: &Type,
    ) -> Result<FieldBuilderPart> {
        let string_ty = string_ty(Span::call_site());
        let ty = option_ty(string_ty.clone());

        Ok(FieldBuilderPart::Init {
            value: FieldTempInit {
                ty,
                init: quote! {
                    ctx.language().map(#string_ty::from).into()
                },
            },
        })
    }

    fn make_iterator_part(
        &self,
        _scope: &AsItemsScope,
        _container_name: &ParentRef,
        bound_name: &Ident,
        _member: &Member,
        ty: &Type,
    ) -> Result<FieldIteratorPart> {
        let as_optional_xml_text = as_optional_xml_text_fn(ty.clone());

        Ok(FieldIteratorPart::Header {
            generator: quote! {
                #as_optional_xml_text(#bound_name)?.map(|#bound_name|
                    ::xso::Item::Attribute(
                        ::xso::exports::rxml::Namespace::XML,
                        // SAFETY: `lang` is a known-good NcName
                        unsafe {
                            ::xso::exports::alloc::borrow::Cow::Borrowed(
                                ::xso::exports::rxml::NcNameStr::from_str_unchecked("lang"),
                            )
                        },
                        #bound_name,
                    )
                )
            },
        })
    }
}
