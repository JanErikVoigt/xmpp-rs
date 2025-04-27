/*!
# Error types for XML parsing

This module contains the error types used throughout the `xso` crate.
*/

// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use core::fmt;

use rxml::Error as XmlError;

/// Opaque string error.
///
/// This is exclusively used in the `From<&Error> for Error` implementation
/// in order to type-erase and "clone" the TextParseError.
///
/// That implementation, in turn, is primarily used by the
/// `AsXml for Result<T, E>` implementation. We intentionally do not implement
/// `Clone` using this type because it'd lose type information (which you
/// don't expect a clone to do).
#[derive(Debug)]
struct OpaqueError(String);

impl fmt::Display for OpaqueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for OpaqueError {}

/// Error variants generated while parsing or serialising XML data.
#[derive(Debug)]
pub enum Error {
    /// Invalid XML data encountered
    XmlError(XmlError),

    /// Attempt to parse text data failed with the provided nested error.
    TextParseError(Box<dyn core::error::Error + Send + Sync + 'static>),

    /// Generic, unspecified other error.
    Other(&'static str),

    /// An element header did not match an expected element.
    ///
    /// This error condition is only generated when the top-level element
    /// passed to [`transform`][`crate::transform`] or similar does not match
    /// the type.
    TypeMismatch,
}

impl Error {
    /// Convenience function to create a [`Self::TextParseError`] variant.
    ///
    /// This includes the `Box::new(.)` call, making it directly usable as
    /// argument to [`Result::map_err`].
    pub fn text_parse_error<T: core::error::Error + Send + Sync + 'static>(e: T) -> Self {
        Self::TextParseError(Box::new(e))
    }
}

/// "Clone" an [`Error`] while discarding some information.
///
/// This discards the specific type information from the
/// [`TextParseError`][`Self::TextParseError`] variant and it may discard
/// more information in the future.
impl From<&Error> for Error {
    fn from(other: &Error) -> Self {
        match other {
            Self::XmlError(e) => Self::XmlError(*e),
            Self::TextParseError(e) => Self::TextParseError(Box::new(OpaqueError(e.to_string()))),
            Self::Other(e) => Self::Other(e),
            Self::TypeMismatch => Self::TypeMismatch,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::XmlError(ref e) => write!(f, "xml parse error: {}", e),
            Self::TextParseError(ref e) => write!(f, "text parse error: {}", e),
            Self::TypeMismatch => f.write_str("mismatch between expected and actual XML data"),
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::XmlError(ref e) => Some(e),
            Self::TextParseError(ref e) => Some(&**e),
            _ => None,
        }
    }
}

impl From<rxml::Error> for Error {
    fn from(other: rxml::Error) -> Error {
        Error::XmlError(other)
    }
}

impl From<rxml::strings::Error> for Error {
    fn from(other: rxml::strings::Error) -> Error {
        Error::XmlError(other.into())
    }
}

impl From<core::convert::Infallible> for Error {
    fn from(other: core::convert::Infallible) -> Self {
        match other {}
    }
}

/// Error returned from
/// [`FromXml::from_events`][`crate::FromXml::from_events`].
#[derive(Debug)]
pub enum FromEventsError {
    /// The `name` and/or `attrs` passed to `FromXml::from_events` did not
    /// match the element's type.
    Mismatch {
        /// The `name` passed to `from_events`.
        name: rxml::QName,

        /// The `attrs` passed to `from_events`.
        attrs: rxml::AttrMap,
    },

    /// The `name` and `attrs` passed to `FromXml::from_events` matched the
    /// element's type, but the data was invalid. Details are in the inner
    /// error.
    Invalid(Error),
}

impl From<Error> for FromEventsError {
    fn from(other: Error) -> Self {
        Self::Invalid(other)
    }
}

impl From<core::convert::Infallible> for FromEventsError {
    fn from(other: core::convert::Infallible) -> Self {
        match other {}
    }
}

impl fmt::Display for FromEventsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Mismatch { .. } => f.write_str("element header did not match"),
            Self::Invalid(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl core::error::Error for FromEventsError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Mismatch { .. } => None,
            Self::Invalid(ref e) => Some(e),
        }
    }
}

impl From<Error> for Result<minidom::Element, Error> {
    fn from(other: Error) -> Self {
        Self::Err(other)
    }
}

/// Error returned by the `TryFrom<Element>` implementations.
#[derive(Debug)]
pub enum FromElementError {
    /// The XML element header did not match the expectations of the type
    /// implementing `TryFrom`.
    ///
    /// Contains the original `Element` unmodified.
    Mismatch(minidom::Element),

    /// During processing of the element, an (unrecoverable) error occurred.
    Invalid(Error),
}

impl fmt::Display for FromElementError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Mismatch(ref el) => write!(
                f,
                "expected different XML element (got {} in namespace {})",
                el.name(),
                el.ns()
            ),
            Self::Invalid(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl core::error::Error for FromElementError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Mismatch(_) => None,
            Self::Invalid(ref e) => Some(e),
        }
    }
}

impl From<Result<minidom::Element, Error>> for FromElementError {
    fn from(other: Result<minidom::Element, Error>) -> Self {
        match other {
            Ok(v) => Self::Mismatch(v),
            Err(e) => Self::Invalid(e),
        }
    }
}

impl From<Error> for FromElementError {
    fn from(other: Error) -> Self {
        Self::Invalid(other)
    }
}

impl From<FromElementError> for Error {
    fn from(other: FromElementError) -> Self {
        match other {
            FromElementError::Invalid(e) => e,
            FromElementError::Mismatch(..) => Self::TypeMismatch,
        }
    }
}

impl From<core::convert::Infallible> for FromElementError {
    fn from(other: core::convert::Infallible) -> Self {
        match other {}
    }
}
