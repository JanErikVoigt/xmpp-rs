// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Definitions common to both enums and structs

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::*;

/// Template which renders to a `xso::fromxml::XmlNameMatcher` value.
pub(crate) enum XmlNameMatcher {
    /// Renders as `xso::fromxml::XmlNameMatcher::Any`.
    #[allow(dead_code)] // We keep it for completeness.
    Any,

    /// Renders as `xso::fromxml::XmlNameMatcher::InNamespace(#0)`.
    InNamespace(TokenStream),

    /// Renders as `xso::fromxml::XmlNameMatcher::Specific(#0, #1)`.
    Specific(TokenStream, TokenStream),

    /// Renders as `#0`.
    ///
    /// This is an escape hatch for more complicated constructs, e.g. when
    /// a superset of multiple matchers is required.
    Custom(TokenStream),
}

impl ToTokens for XmlNameMatcher {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Any => tokens.extend(quote! {
                ::xso::fromxml::XmlNameMatcher::<'static>::Any
            }),
            Self::InNamespace(ref namespace) => tokens.extend(quote! {
                ::xso::fromxml::XmlNameMatcher::<'static>::InNamespace(#namespace)
            }),
            Self::Specific(ref namespace, ref name) => tokens.extend(quote! {
                ::xso::fromxml::XmlNameMatcher::<'static>::Specific(#namespace, #name)
            }),
            Self::Custom(ref stream) => tokens.extend(stream.clone()),
        }
    }
}

/// Parts necessary to construct a `::xso::FromXml` implementation.
pub(crate) struct FromXmlParts {
    /// Additional items necessary for the implementation.
    pub(crate) defs: TokenStream,

    /// The body of the `::xso::FromXml::from_xml` function.
    pub(crate) from_events_body: TokenStream,

    /// The name of the type which is the `::xso::FromXml::Builder`.
    pub(crate) builder_ty_ident: Ident,

    /// The `XmlNameMatcher` to pre-select elements for this implementation.
    pub(crate) name_matcher: XmlNameMatcher,
}

/// Parts necessary to construct a `::xso::AsXml` implementation.
pub(crate) struct AsXmlParts {
    /// Additional items necessary for the implementation.
    pub(crate) defs: TokenStream,

    /// The body of the `::xso::AsXml::as_xml_iter` function.
    pub(crate) as_xml_iter_body: TokenStream,

    /// The type which is the `::xso::AsXml::ItemIter`.
    pub(crate) item_iter_ty: Type,

    /// The lifetime name used in `item_iter_ty`.
    pub(crate) item_iter_ty_lifetime: Lifetime,
}

/// Trait describing the definition of the XML (de-)serialisation for an item
/// (enum or struct).
pub(crate) trait ItemDef {
    /// Construct the parts necessary for the caller to build an
    /// `xso::FromXml` implementation for the item.
    fn make_from_events_builder(
        &self,
        vis: &Visibility,
        name_ident: &Ident,
        attrs_ident: &Ident,
    ) -> Result<FromXmlParts>;

    /// Construct the parts necessary for the caller to build an `xso::AsXml`
    /// implementation for the item.
    fn make_as_xml_iter(&self, vis: &Visibility) -> Result<AsXmlParts>;

    /// Return true iff the user requested debug output.
    fn debug(&self) -> bool;
}
