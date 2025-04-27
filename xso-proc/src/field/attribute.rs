// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module concerns the processing of attributes.
//!
//! In particular, it provides the `#[xml(attribute)]` implementation.

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::*;

use std::borrow::Cow;

use crate::error_message::{self, ParentRef};
use crate::meta::{Flag, NameRef, NamespaceRef, QNameRef, XMLNS_XML};
use crate::scope::{AsItemsScope, FromEventsScope};
use crate::types::{
    as_optional_xml_text_fn, default_fn, from_xml_text_fn, text_codec_decode_fn,
    text_codec_encode_fn,
};

use super::{Field, FieldBuilderPart, FieldIteratorPart, FieldTempInit};

/// Subtype for attribute-matching fields.
pub(super) enum AttributeFieldKind {
    /// Matches any attribute
    Generic {
        /// The optional XML namespace of the attribute.
        xml_namespace: Option<NamespaceRef>,

        /// The XML name of the attribute.
        xml_name: NameRef,
    },

    /// Matches `xml:lang`
    XmlLang,
}

impl AttributeFieldKind {
    fn matcher(&self) -> (Cow<'_, Option<NamespaceRef>>, Cow<'_, NameRef>) {
        match self {
            Self::Generic {
                ref xml_namespace,
                ref xml_name,
            } => (Cow::Borrowed(xml_namespace), Cow::Borrowed(xml_name)),
            Self::XmlLang => (
                Cow::Owned(Some(NamespaceRef::fudge(XMLNS_XML, Span::call_site()))),
                Cow::Owned(NameRef::fudge(
                    rxml_validation::NcName::try_from("lang").unwrap(),
                    Span::call_site(),
                )),
            ),
        }
    }

    fn qname_ref(&self) -> QNameRef {
        let (namespace, name) = self.matcher();
        QNameRef {
            namespace: namespace.into_owned(),
            name: Some(name.into_owned()),
        }
    }
}

/// The field maps to an attribute.
pub(super) struct AttributeField {
    /// Subtype
    pub(super) kind: AttributeFieldKind,

    /// Flag indicating whether the value should be defaulted if the
    /// attribute is absent.
    pub(super) default_: Flag,

    /// Optional codec to use.
    pub(super) codec: Option<Expr>,
}

impl Field for AttributeField {
    fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldBuilderPart> {
        let FromEventsScope { ref attrs, .. } = scope;
        let ty = ty.clone();

        let fetch = match self.kind {
            AttributeFieldKind::Generic {
                ref xml_namespace,
                ref xml_name,
            } => {
                let xml_namespace = match xml_namespace {
                    Some(v) => v.to_token_stream(),
                    None => quote! {
                        ::xso::exports::rxml::Namespace::none()
                    },
                };

                quote! {
                    #attrs.remove(#xml_namespace, #xml_name)
                }
            }

            AttributeFieldKind::XmlLang => {
                quote! {
                    ctx.language().map(::xso::exports::alloc::borrow::ToOwned::to_owned)
                }
            }
        };

        let finalize = match self.codec {
            Some(ref codec) => {
                let decode = text_codec_decode_fn(ty.clone());
                quote! {
                    |value| #decode(&#codec, value)
                }
            }
            None => {
                let from_xml_text = from_xml_text_fn(ty.clone());
                quote! { #from_xml_text }
            }
        };

        let missing_msg = error_message::on_missing_attribute(container_name, member);
        let on_absent = match self.default_ {
            Flag::Absent => quote! {
                return ::core::result::Result::Err(::xso::error::Error::Other(#missing_msg).into())
            },
            Flag::Present(_) => {
                let default_ = default_fn(ty.clone());
                quote! {
                    #default_()
                }
            }
        };

        Ok(FieldBuilderPart::Init {
            value: FieldTempInit {
                init: quote! {
                    match #fetch.map(#finalize).transpose()? {
                        ::core::option::Option::Some(v) => v,
                        ::core::option::Option::None => #on_absent,
                    }
                },
                ty: ty.clone(),
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
        let (xml_namespace, xml_name) = self.kind.matcher();
        let xml_namespace = match xml_namespace.as_ref() {
            Some(ref v) => quote! { ::xso::exports::rxml::Namespace::from(#v) },
            None => quote! {
                ::xso::exports::rxml::Namespace::NONE
            },
        };

        let generator = match self.codec {
            Some(ref codec) => {
                let encode = text_codec_encode_fn(ty.clone());
                quote! { #encode(&#codec, #bound_name)? }
            }
            None => {
                let as_optional_xml_text = as_optional_xml_text_fn(ty.clone());
                quote! { #as_optional_xml_text(#bound_name)? }
            }
        };

        Ok(FieldIteratorPart::Header {
            generator: quote! {
                #generator.map(|#bound_name| ::xso::Item::Attribute(
                    #xml_namespace,
                    ::xso::exports::alloc::borrow::Cow::Borrowed(#xml_name),
                    #bound_name,
                ));
            },
        })
    }

    fn captures_attribute(&self) -> Option<QNameRef> {
        Some(self.kind.qname_ref())
    }
}
