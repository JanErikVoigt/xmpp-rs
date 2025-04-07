// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Module containing implementations for conversions to/from XML text.

use core::marker::PhantomData;

use alloc::{
    borrow::Cow,
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::{error::Error, AsXmlText, FromXmlText};

#[cfg(feature = "base64")]
use base64::engine::general_purpose::STANDARD as StandardBase64Engine;

macro_rules! convert_via_fromstr_and_display {
    ($($(#[cfg $cfg:tt])?$t:ty,)+) => {
        $(
            $(
                #[cfg $cfg]
            )?
            impl FromXmlText for $t {
                #[doc = concat!("Parse [`", stringify!($t), "`] from XML text via [`FromStr`][`core::str::FromStr`].")]
                fn from_xml_text(s: String) -> Result<Self, Error> {
                    s.parse().map_err(Error::text_parse_error)
                }
            }

            $(
                #[cfg $cfg]
            )?
            impl AsXmlText for $t {
                #[doc = concat!("Convert [`", stringify!($t), "`] to XML text via [`Display`][`core::fmt::Display`].\n\nThis implementation never fails.")]
                fn as_xml_text(&self) -> Result<Cow<'_, str>, Error> {
                    Ok(Cow::Owned(self.to_string()))
                }
            }
        )+
    }
}

/// This provides an implementation compliant with xsd::bool.
impl FromXmlText for bool {
    /// Parse a boolean from XML text.
    ///
    /// The values `"1"` and `"true"` are considered true. The values `"0"`
    /// and `"false"` are considered `false`. Any other value is invalid and
    /// will return an error.
    fn from_xml_text(s: String) -> Result<Self, Error> {
        match s.as_str() {
            "1" => "true",
            "0" => "false",
            other => other,
        }
        .parse()
        .map_err(Error::text_parse_error)
    }
}

/// This provides an implementation compliant with xsd::bool.
impl AsXmlText for bool {
    /// Convert a boolean to XML text.
    ///
    /// `true` is converted to `"true"` and `false` is converted to `"false"`.
    /// This implementation never fails.
    fn as_xml_text(&self) -> Result<Cow<'_, str>, Error> {
        match self {
            true => Ok(Cow::Borrowed("true")),
            false => Ok(Cow::Borrowed("false")),
        }
    }
}

convert_via_fromstr_and_display! {
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    char,
    core::net::IpAddr,
    core::net::Ipv4Addr,
    core::net::Ipv6Addr,
    core::net::SocketAddr,
    core::net::SocketAddrV4,
    core::net::SocketAddrV6,
    core::num::NonZeroU8,
    core::num::NonZeroU16,
    core::num::NonZeroU32,
    core::num::NonZeroU64,
    core::num::NonZeroU128,
    core::num::NonZeroUsize,
    core::num::NonZeroI8,
    core::num::NonZeroI16,
    core::num::NonZeroI32,
    core::num::NonZeroI64,
    core::num::NonZeroI128,
    core::num::NonZeroIsize,

    #[cfg(feature = "uuid")]
    uuid::Uuid,

    #[cfg(feature = "jid")]
    jid::Jid,
    #[cfg(feature = "jid")]
    jid::FullJid,
    #[cfg(feature = "jid")]
    jid::BareJid,
    #[cfg(feature = "jid")]
    jid::NodePart,
    #[cfg(feature = "jid")]
    jid::DomainPart,
    #[cfg(feature = "jid")]
    jid::ResourcePart,

    #[cfg(feature = "serde_json")]
    serde_json::Value,
}

/// Represent a way to encode/decode text data into a Rust type.
///
/// This trait can be used in scenarios where implementing [`FromXmlText`]
/// and/or [`AsXmlText`] on a type is not feasible or sensible, such as the
/// following:
///
/// 1. The type originates in a foreign crate, preventing the implementation
///    of foreign traits.
///
/// 2. There is more than one way to convert a value to/from XML.
///
/// The codec to use for a text can be specified in the attributes understood
/// by `FromXml` and `AsXml` derive macros. See the documentation of the
/// [`FromXml`][`macro@crate::FromXml`] derive macro for details.
pub trait TextCodec<T> {
    /// Decode a string value into the type.
    fn decode(&self, s: String) -> Result<T, Error>;

    /// Encode the type as string value.
    ///
    /// If this returns `None`, the string value is not emitted at all.
    fn encode<'x>(&self, value: &'x T) -> Result<Option<Cow<'x, str>>, Error>;

    /// Apply a filter to this codec.
    ///
    /// Filters preprocess strings before they are handed to the codec for
    /// parsing, allowing to, for example, make the codec ignore irrelevant
    /// content by stripping it.
    // NOTE: The bound on T is needed because any given type A may implement
    // TextCodec for any number of types. If we pass T down to the `Filtered`
    // struct, rustc can do type inference on which `TextCodec`
    // implementation the `filtered` method is supposed to have been called
    // on.
    fn filtered<F: TextFilter>(self, filter: F) -> Filtered<F, Self, T>
    where
        // placing the bound here (instead of on the `TextCodec<T>` trait
        // itself) preserves object-safety of TextCodec<T>.
        Self: Sized,
    {
        Filtered {
            filter,
            codec: self,
            bound: PhantomData,
        }
    }
}

/// Wrapper struct to apply a filter to a codec.
///
/// You can construct a value of this type via [`TextCodec::filtered`].
// NOTE: see the note on TextCodec::filtered for why we bind `T` here, too.
pub struct Filtered<F, C, T> {
    filter: F,
    codec: C,
    bound: PhantomData<T>,
}

impl<T, F: TextFilter, C: TextCodec<T>> TextCodec<T> for Filtered<F, C, T> {
    fn decode(&self, s: String) -> Result<T, Error> {
        let s = self.filter.preprocess(s);
        self.codec.decode(s)
    }

    fn encode<'x>(&self, value: &'x T) -> Result<Option<Cow<'x, str>>, Error> {
        self.codec.encode(value)
    }
}

/// Text codec which does no transform.
pub struct Plain;

impl TextCodec<String> for Plain {
    fn decode(&self, s: String) -> Result<String, Error> {
        Ok(s)
    }

    fn encode<'x>(&self, value: &'x String) -> Result<Option<Cow<'x, str>>, Error> {
        Ok(Some(Cow::Borrowed(value.as_str())))
    }
}

/// Text codec which returns `None` if the input to decode is the empty string, instead of
/// attempting to decode it.
///
/// Particularly useful when parsing `Option<T>` on `#[xml(text)]`, which does not support
/// `Option<_>` otherwise.
pub struct EmptyAsNone;

impl<T> TextCodec<Option<T>> for EmptyAsNone
where
    T: FromXmlText + AsXmlText,
{
    fn decode(&self, s: String) -> Result<Option<T>, Error> {
        if s.is_empty() {
            Ok(None)
        } else {
            Some(T::from_xml_text(s)).transpose()
        }
    }

    fn encode<'x>(&self, value: &'x Option<T>) -> Result<Option<Cow<'x, str>>, Error> {
        Ok(value
            .as_ref()
            .map(AsXmlText::as_xml_text)
            .transpose()?
            .and_then(|v| (!v.is_empty()).then_some(v)))
    }
}

/// Text codec which returns None instead of the empty string.
pub struct EmptyAsError;

impl TextCodec<String> for EmptyAsError {
    fn decode(&self, s: String) -> Result<String, Error> {
        if s.is_empty() {
            Err(Error::Other("Empty text node."))
        } else {
            Ok(s)
        }
    }

    fn encode<'x>(&self, value: &'x String) -> Result<Option<Cow<'x, str>>, Error> {
        if value.is_empty() {
            Err(Error::Other("Empty text node."))
        } else {
            Ok(Some(Cow::Borrowed(value.as_str())))
        }
    }
}

/// Trait for preprocessing text data from XML.
///
/// This may be used by codecs to allow to customize some of their behaviour.
pub trait TextFilter {
    /// Process the incoming string and return the result of the processing.
    fn preprocess(&self, s: String) -> String;
}

/// Text preprocessor which returns the input unchanged.
pub struct NoFilter;

impl TextFilter for NoFilter {
    fn preprocess(&self, s: String) -> String {
        s
    }
}

/// Text preprocessor to remove all whitespace.
pub struct StripWhitespace;

impl TextFilter for StripWhitespace {
    fn preprocess(&self, s: String) -> String {
        let s: String = s
            .chars()
            .filter(|ch| *ch != ' ' && *ch != '\n' && *ch != '\t')
            .collect();
        s
    }
}

/// Text codec transforming text to binary using standard `base64`.
///
/// `Base64` uses the [`base64::engine::general_purpose::STANDARD`] engine.
/// [`TextCodec`] is also automatically implemented for any value which
/// implements [`base64::engine::Engine`], allowing you to choose different
/// alphabets easily.
#[cfg(feature = "base64")]
pub struct Base64;

#[cfg(feature = "base64")]
impl TextCodec<Vec<u8>> for Base64 {
    fn decode(&self, s: String) -> Result<Vec<u8>, Error> {
        base64::engine::Engine::decode(&StandardBase64Engine, s.as_bytes())
            .map_err(Error::text_parse_error)
    }

    fn encode<'x>(&self, value: &'x Vec<u8>) -> Result<Option<Cow<'x, str>>, Error> {
        Ok(Some(Cow::Owned(base64::engine::Engine::encode(
            &StandardBase64Engine,
            value,
        ))))
    }
}

#[cfg(feature = "base64")]
impl<'x> TextCodec<Cow<'x, [u8]>> for Base64 {
    fn decode(&self, s: String) -> Result<Cow<'x, [u8]>, Error> {
        base64::engine::Engine::decode(&StandardBase64Engine, s.as_bytes())
            .map_err(Error::text_parse_error)
            .map(Cow::Owned)
    }

    fn encode<'a>(&self, value: &'a Cow<'x, [u8]>) -> Result<Option<Cow<'a, str>>, Error> {
        Ok(Some(Cow::Owned(base64::engine::Engine::encode(
            &StandardBase64Engine,
            value,
        ))))
    }
}

#[cfg(feature = "base64")]
impl<T> TextCodec<Option<T>> for Base64
where
    Base64: TextCodec<T>,
{
    fn decode(&self, s: String) -> Result<Option<T>, Error> {
        if s.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.decode(s)?))
    }

    fn encode<'x>(&self, decoded: &'x Option<T>) -> Result<Option<Cow<'x, str>>, Error> {
        decoded
            .as_ref()
            .map(|x| self.encode(x))
            .transpose()
            .map(Option::flatten)
    }
}

#[cfg(feature = "base64")]
impl<T: base64::engine::Engine> TextCodec<Vec<u8>> for T {
    fn decode(&self, s: String) -> Result<Vec<u8>, Error> {
        base64::engine::Engine::decode(self, s.as_bytes()).map_err(Error::text_parse_error)
    }

    fn encode<'x>(&self, value: &'x Vec<u8>) -> Result<Option<Cow<'x, str>>, Error> {
        Ok(Some(Cow::Owned(base64::engine::Engine::encode(
            self, value,
        ))))
    }
}

#[cfg(feature = "base64")]
impl<T: base64::engine::Engine, U> TextCodec<Option<U>> for T
where
    T: TextCodec<U>,
{
    fn decode(&self, s: String) -> Result<Option<U>, Error> {
        if s.is_empty() {
            return Ok(None);
        }
        Ok(Some(TextCodec::decode(self, s)?))
    }

    fn encode<'x>(&self, decoded: &'x Option<U>) -> Result<Option<Cow<'x, str>>, Error> {
        decoded
            .as_ref()
            .map(|x| TextCodec::encode(self, x))
            .transpose()
            .map(Option::flatten)
    }
}

/// Text codec transforming text to binary using hexadecimal nibbles.
///
/// The length must be known at compile-time.
pub struct FixedHex<const N: usize>;

impl<const N: usize> TextCodec<[u8; N]> for FixedHex<N> {
    fn decode(&self, s: String) -> Result<[u8; N], Error> {
        if s.len() != 2 * N {
            return Err(Error::Other("Invalid length"));
        }

        let mut bytes = [0u8; N];
        for i in 0..N {
            bytes[i] =
                u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(Error::text_parse_error)?;
        }

        Ok(bytes)
    }

    fn encode<'x>(&self, value: &'x [u8; N]) -> Result<Option<Cow<'x, str>>, Error> {
        let mut bytes = String::with_capacity(N * 2);
        for byte in value {
            bytes.extend(format!("{:02x}", byte).chars());
        }
        Ok(Some(Cow::Owned(bytes)))
    }
}

impl<T, const N: usize> TextCodec<Option<T>> for FixedHex<N>
where
    FixedHex<N>: TextCodec<T>,
{
    fn decode(&self, s: String) -> Result<Option<T>, Error> {
        if s.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.decode(s)?))
    }

    fn encode<'x>(&self, decoded: &'x Option<T>) -> Result<Option<Cow<'x, str>>, Error> {
        decoded
            .as_ref()
            .map(|x| self.encode(x))
            .transpose()
            .map(Option::flatten)
    }
}

/// Text codec for colon-separated bytes of uppercase hexadecimal.
pub struct ColonSeparatedHex;

impl TextCodec<Vec<u8>> for ColonSeparatedHex {
    fn decode(&self, s: String) -> Result<Vec<u8>, Error> {
        assert_eq!((s.len() + 1) % 3, 0);
        let mut bytes = Vec::with_capacity((s.len() + 1) / 3);
        for i in 0..(1 + s.len()) / 3 {
            let byte =
                u8::from_str_radix(&s[3 * i..3 * i + 2], 16).map_err(Error::text_parse_error)?;
            if 3 * i + 2 < s.len() {
                assert_eq!(&s[3 * i + 2..3 * i + 3], ":");
            }
            bytes.push(byte);
        }
        Ok(bytes)
    }

    fn encode<'x>(&self, decoded: &'x Vec<u8>) -> Result<Option<Cow<'x, str>>, Error> {
        // TODO: Super inefficient!
        let mut bytes = Vec::with_capacity(decoded.len());
        for byte in decoded {
            bytes.push(format!("{:02X}", byte));
        }
        Ok(Some(Cow::Owned(bytes.join(":"))))
    }
}
