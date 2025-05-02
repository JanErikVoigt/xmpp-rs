// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module concerns the processing of text content.
//!
//! In particular, it provides the `#[xml(text)]` implementation.

use proc_macro2::Span;
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, *};

use crate::error_message::ParentRef;
use crate::scope::{AsItemsScope, FromEventsScope};
use crate::types::{
    as_xml_text_fn, from_xml_text_fn, string_ty, text_codec_decode_fn, text_codec_encode_fn,
};

use super::{Field, FieldBuilderPart, FieldIteratorPart, FieldTempInit};

/// The field maps to the character data of the element.
pub(super) struct TextField {
    /// Optional codec to use
    pub(super) codec: Option<Expr>,
}

impl Field for TextField {
    fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        _container_name: &ParentRef,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldBuilderPart> {
        let FromEventsScope { ref text, .. } = scope;
        let field_access = scope.access_field(member);
        let finalize = match self.codec {
            Some(ref codec) => {
                let span = codec.span();
                let decode = text_codec_decode_fn(ty.clone(), span);
                quote_spanned! { span=>
                    #decode(&#codec, #field_access)?
                }
            }
            None => {
                let from_xml_text = from_xml_text_fn(ty.clone());
                quote! { #from_xml_text(#field_access)? }
            }
        };

        Ok(FieldBuilderPart::Text {
            value: FieldTempInit {
                init: quote! { ::xso::exports::alloc::string::String::new() },
                ty: string_ty(Span::call_site()),
            },
            collect: quote! {
                #field_access.push_str(#text.as_str());
            },
            finalize,
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
        let generator = match self.codec {
            Some(ref codec) => {
                let span = codec.span();
                let encode = text_codec_encode_fn(ty.clone(), span);
                // NOTE: We need to fudge the span of `bound_name` here,
                // because its span points outside the macro (the identifier
                // of the field), which means that quote_spanned will not
                // override it, which would make the error message ugly.
                let mut bound_name = bound_name.clone();
                bound_name.set_span(span);
                quote_spanned! { span=> #encode(&#codec, #bound_name)? }
            }
            None => {
                let as_xml_text = as_xml_text_fn(ty.clone());
                quote! { ::core::option::Option::Some(#as_xml_text(#bound_name)?) }
            }
        };

        Ok(FieldIteratorPart::Text { generator })
    }

    fn captures_text(&self) -> bool {
        true
    }
}
