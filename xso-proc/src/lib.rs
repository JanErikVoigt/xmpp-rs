// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(rustdoc::private_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
/*!
# Macros for parsing XML into Rust structs, and vice versa

**If you are a user of `xso_proc` or `xso`, please
return to `xso` for more information**. The documentation of
`xso_proc` is geared toward developers of `…_macros` and `…_core`.

**You have been warned.**
*/

// Wondering about RawTokenStream vs. TokenStream?
// syn mostly works with proc_macro2, while the proc macros themselves use
// proc_macro.
use proc_macro::TokenStream as RawTokenStream;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::*;

mod common;
mod compound;
mod enums;
mod error_message;
mod field;
mod meta;
mod scope;
mod state;
mod structs;
mod types;

use common::{AsXmlParts, FromXmlParts, ItemDef};

/// Convert an [`syn::Item`] into the parts relevant for us.
///
/// If the item is of an unsupported variant, an appropriate error is
/// returned.
fn parse_struct(item: Item) -> Result<(Visibility, Ident, Box<dyn ItemDef>)> {
    match item {
        Item::Struct(item) => {
            let meta = meta::XmlCompoundMeta::parse_from_attributes(&item.attrs)?;
            let def = structs::StructDef::new(&item.ident, meta, &item.fields)?;
            Ok((item.vis, item.ident, Box::new(def)))
        }
        Item::Enum(item) => {
            let meta = meta::XmlCompoundMeta::parse_from_attributes(&item.attrs)?;
            let def = enums::EnumDef::new(&item.ident, meta, &item.variants)?;
            Ok((item.vis, item.ident, Box::new(def)))
        }
        other => Err(Error::new_spanned(other, "cannot derive on this item")),
    }
}

/// Generate a `xso::FromXml` implementation for the given item, or fail with
/// a proper compiler error.
fn from_xml_impl(input: Item) -> Result<TokenStream> {
    let (vis, ident, def) = parse_struct(input)?;

    let name_ident = Ident::new("name", Span::call_site());
    let attrs_ident = Ident::new("attrs", Span::call_site());

    let FromXmlParts {
        defs,
        from_events_body,
        builder_ty_ident,
    } = def.make_from_events_builder(&vis, &name_ident, &attrs_ident)?;

    #[cfg_attr(not(feature = "minidom"), allow(unused_mut))]
    let mut result = quote! {
        #defs

        impl ::xso::FromXml for #ident {
            type Builder = #builder_ty_ident;

            fn from_events(
                name: ::xso::exports::rxml::QName,
                attrs: ::xso::exports::rxml::AttrMap,
                ctx: &::xso::Context<'_>,
            ) -> ::core::result::Result<Self::Builder, ::xso::error::FromEventsError> {
                #from_events_body
            }
        }
    };

    #[cfg(feature = "minidom")]
    result.extend(quote! {
        impl ::core::convert::TryFrom<::xso::exports::minidom::Element> for #ident {
            type Error = ::xso::error::FromElementError;

            fn try_from(other: ::xso::exports::minidom::Element) -> ::core::result::Result<Self, ::xso::error::FromElementError> {
                ::xso::try_from_element(other)
            }
        }
    });

    if def.debug() {
        println!("{}", result);
    }

    Ok(result)
}

/// Macro to derive a `xso::FromXml` implementation on a type.
///
/// The user-facing documentation for this macro lives in the `xso` crate.
#[proc_macro_derive(FromXml, attributes(xml))]
pub fn from_xml(input: RawTokenStream) -> RawTokenStream {
    // Shim wrapper around `from_xml_impl` which converts any errors into
    // actual compiler errors within the resulting token stream.
    let item = syn::parse_macro_input!(input as Item);
    match from_xml_impl(item) {
        Ok(v) => v.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// Generate a `xso::AsXml` implementation for the given item, or fail with
/// a proper compiler error.
fn as_xml_impl(input: Item) -> Result<TokenStream> {
    let (vis, ident, def) = parse_struct(input)?;

    let AsXmlParts {
        defs,
        as_xml_iter_body,
        item_iter_ty_lifetime,
        item_iter_ty,
    } = def.make_as_xml_iter(&vis)?;

    #[cfg_attr(not(feature = "minidom"), allow(unused_mut))]
    let mut result = quote! {
        #defs

        impl ::xso::AsXml for #ident {
            type ItemIter<#item_iter_ty_lifetime> = #item_iter_ty;

            fn as_xml_iter(&self) -> ::core::result::Result<Self::ItemIter<'_>, ::xso::error::Error> {
                #as_xml_iter_body
            }
        }
    };

    #[cfg(all(feature = "minidom", feature = "panicking-into-impl"))]
    result.extend(quote! {
        impl ::core::convert::From<#ident> for ::xso::exports::minidom::Element {
            fn from(other: #ident) -> Self {
                ::xso::transform(&other).expect("seamless conversion into minidom::Element")
            }
        }

        impl ::core::convert::From<&#ident> for ::xso::exports::minidom::Element {
            fn from(other: &#ident) -> Self {
                ::xso::transform(other).expect("seamless conversion into minidom::Element")
            }
        }
    });

    #[cfg(all(feature = "minidom", not(feature = "panicking-into-impl")))]
    result.extend(quote! {
        impl ::core::convert::TryFrom<#ident> for ::xso::exports::minidom::Element {
            type Error = ::xso::error::Error;

            fn try_from(other: #ident) -> ::core::result::Result<Self, Self::Error> {
                ::xso::transform(&other)
            }
        }
        impl ::core::convert::TryFrom<&#ident> for ::xso::exports::minidom::Element {
            type Error = ::xso::error::Error;

            fn try_from(other: &#ident) -> ::core::result::Result<Self, Self::Error> {
                ::xso::transform(other)
            }
        }
    });

    if def.debug() {
        println!("{}", result);
    }

    Ok(result)
}

/// Macro to derive a `xso::AsXml` implementation on a type.
///
/// The user-facing documentation for this macro lives in the `xso` crate.
#[proc_macro_derive(AsXml, attributes(xml))]
pub fn as_xml(input: RawTokenStream) -> RawTokenStream {
    // Shim wrapper around `as_xml_impl` which converts any errors into
    // actual compiler errors within the resulting token stream.
    let item = syn::parse_macro_input!(input as Item);
    match as_xml_impl(item) {
        Ok(v) => v.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
