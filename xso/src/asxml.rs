//! # Generic iterator type implementations
//!
//! This module contains [`AsXml`] iterator implementations for types from
//! foreign libraries (such as the standard library).
//!
//! In order to not clutter the `xso` crate's main namespace, they are
//! stashed away in a separate module.

// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use alloc::boxed::Box;

use crate::error::Error;
use crate::rxml_util::Item;
use crate::AsXml;

use core::fmt;

use bytes::BytesMut;

/// Helper iterator to convert an `Option<T>` to XML.
pub struct OptionAsXml<T: Iterator>(Option<T>);

impl<T: Iterator> OptionAsXml<T> {
    /// Construct a new iterator, wrapping the given iterator.
    ///
    /// If `inner` is `None`, this iterator terminates immediately. Otherwise,
    /// it yields the elements yielded by `inner` until `inner` finishes,
    /// after which this iterator completes, too.
    pub fn new(inner: Option<T>) -> Self {
        Self(inner)
    }
}

impl<'x, T: Iterator<Item = Result<Item<'x>, Error>>> Iterator for OptionAsXml<T> {
    type Item = Result<Item<'x>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut()?.next()
    }
}

/// Emits the contents of `Some(.)` unchanged if present and nothing
/// otherwise.
impl<T: AsXml> AsXml for Option<T> {
    type ItemIter<'x>
        = OptionAsXml<T::ItemIter<'x>>
    where
        T: 'x;

    fn as_xml_iter(&self) -> Result<Self::ItemIter<'_>, Error> {
        match self {
            Some(ref value) => Ok(OptionAsXml(Some(T::as_xml_iter(value)?))),
            None => Ok(OptionAsXml(None)),
        }
    }
}

/// Helper iterator to convert an `Box<T>` to XML.
pub struct BoxAsXml<T: Iterator>(Box<T>);

impl<'x, T: Iterator<Item = Result<Item<'x>, Error>>> Iterator for BoxAsXml<T> {
    type Item = Result<Item<'x>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Emits the contents of `T` unchanged.
impl<T: AsXml> AsXml for Box<T> {
    type ItemIter<'x>
        = BoxAsXml<T::ItemIter<'x>>
    where
        T: 'x;

    fn as_xml_iter(&self) -> Result<Self::ItemIter<'_>, Error> {
        Ok(BoxAsXml(Box::new(T::as_xml_iter(self)?)))
    }
}

/// Emits the items of `T` if `Ok(.)` or returns the error from `E` otherwise.
impl<T: AsXml, E> AsXml for Result<T, E>
where
    for<'a> Error: From<&'a E>,
{
    type ItemIter<'x>
        = T::ItemIter<'x>
    where
        Self: 'x;

    fn as_xml_iter(&self) -> Result<Self::ItemIter<'_>, Error> {
        match self {
            Self::Ok(v) => Ok(v.as_xml_iter()?),
            Self::Err(e) => Err(e.into()),
        }
    }
}

/// Provides a helper which implements Display printing raw XML
pub struct PrintRawXml<'x, T>(pub &'x T);

impl<T: AsXml> fmt::Display for PrintRawXml<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = match self.0.as_xml_iter() {
            Ok(iter) => iter,
            Err(err) => return write!(f, "<failed to serialize PrintRawXml: {:?}>", err),
        };
        let mut writer = rxml::writer::Encoder::new();
        let mut buf = BytesMut::new();
        for item in iter {
            let item = match item {
                Ok(item) => item,
                Err(err) => return write!(f, "<failed to serialize PrintRawXml: {:?}>", err),
            };
            if let Err(err) = writer.encode(item.as_rxml_item(), &mut buf) {
                return write!(f, "<failed to serialize PrintRawXml: {:?}>", err);
            }
        }
        // TODO: rxml guarantees us that we have utf8 here. This unwrap can nonetheless be removed
        // if Write is implemented for rxml.
        write!(f, "{}", std::str::from_utf8(&buf).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::{borrow::Cow, vec};

    #[test]
    fn option_as_xml_terminates_immediately_for_none() {
        let mut iter = OptionAsXml::<core::iter::Empty<_>>(None);
        match iter.next() {
            None => (),
            other => panic!("unexpected item: {:?}", other),
        }
    }

    #[test]
    fn option_as_xml_passes_values_from_inner_some() {
        let inner = vec![
            Ok(Item::Text(Cow::Borrowed("hello world"))),
            Ok(Item::ElementFoot),
        ];
        let mut iter = OptionAsXml(Some(inner.into_iter()));
        match iter.next() {
            Some(Ok(Item::Text(text))) => {
                assert_eq!(text, "hello world");
            }
            other => panic!("unexpected item: {:?}", other),
        }
        match iter.next() {
            Some(Ok(Item::ElementFoot)) => (),
            other => panic!("unexpected item: {:?}", other),
        }
        match iter.next() {
            None => (),
            other => panic!("unexpected item: {:?}", other),
        }
    }

    #[test]
    fn box_as_xml_passes_values_from_inner() {
        let inner = vec![
            Ok(Item::Text(Cow::Borrowed("hello world"))),
            Ok(Item::ElementFoot),
        ];
        let mut iter = BoxAsXml(Box::new(inner.into_iter()));
        match iter.next() {
            Some(Ok(Item::Text(text))) => {
                assert_eq!(text, "hello world");
            }
            other => panic!("unexpected item: {:?}", other),
        }
        match iter.next() {
            Some(Ok(Item::ElementFoot)) => (),
            other => panic!("unexpected item: {:?}", other),
        }
        match iter.next() {
            None => (),
            other => panic!("unexpected item: {:?}", other),
        }
    }
}
