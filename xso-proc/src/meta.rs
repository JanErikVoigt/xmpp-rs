// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! # Parse Rust attributes
//!
//! This module is concerned with parsing attributes from the Rust "meta"
//! annotations on structs, enums, enum variants and fields.

use core::hash::{Hash, Hasher};

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{meta::ParseNestedMeta, spanned::Spanned, *};

use rxml_validation::NcName;

/// XML core namespace URI (for the `xml:` prefix)
pub const XMLNS_XML: &str = "http://www.w3.org/XML/1998/namespace";
/// XML namespace URI (for the `xmlns:` prefix)
pub const XMLNS_XMLNS: &str = "http://www.w3.org/2000/xmlns/";

macro_rules! reject_key {
    ($key:ident not on $not_allowed_on:literal $(only on $only_allowed_on:literal)?) => {
        if let Some(ref $key) = $key {
            return Err(Error::new_spanned(
                $key,
                concat!(
                    "`",
                    stringify!($key),
                    "` is not allowed on ",
                    $not_allowed_on,
                    $(
                        " (only on ",
                        $only_allowed_on,
                        ")",
                    )?
                ),
            ));
        }
    };

    ($key:ident flag not on $not_allowed_on:literal $(only on $only_allowed_on:literal)?) => {
        if let Flag::Present(ref $key) = $key {
            return Err(Error::new(
                *$key,
                concat!(
                    "`",
                    stringify!($key),
                    "` is not allowed on ",
                    $not_allowed_on,
                    $(
                        " (only on ",
                        $only_allowed_on,
                        ")",
                    )?
                ),
            ));
        }
    };

    ($key:ident vec not on $not_allowed_on:literal $(only on $only_allowed_on:literal)?) => {
        if let Some(ref $key) = $key.first() {
            return Err(Error::new(
                $key.span(),
                concat!(
                    "`",
                    stringify!($key),
                    "` is not allowed on ",
                    $not_allowed_on,
                    $(
                        " (only on ",
                        $only_allowed_on,
                        ")",
                    )?
                ),
            ));
        }
    };
}

pub(crate) use reject_key;

/// Value for the `#[xml(namespace = ..)]` attribute.
#[derive(Debug, Clone)]
pub(crate) enum NamespaceRef {
    /// The XML namespace is specified as a string literal.
    LitStr(LitStr),

    /// The XML namespace is specified as a path.
    Path(Path),
}

impl NamespaceRef {
    fn fudge(value: &str, span: Span) -> Self {
        Self::LitStr(LitStr::new(value, span))
    }
}

impl syn::parse::Parse for NamespaceRef {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        if input.peek(syn::LitStr) {
            Ok(Self::LitStr(input.parse()?))
        } else {
            Ok(Self::Path(input.parse()?))
        }
    }
}

impl quote::ToTokens for NamespaceRef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::LitStr(ref lit) => lit.to_tokens(tokens),
            Self::Path(ref path) => path.to_tokens(tokens),
        }
    }
}

/// Value for the `#[xml(name = .. )]` attribute.
#[derive(Debug, Clone)]
pub(crate) enum NameRef {
    /// The XML name is specified as a string literal.
    Literal {
        /// The validated XML name.
        value: NcName,

        /// The span of the original [`syn::LitStr`].
        span: Span,
    },

    /// The XML name is specified as a path.
    Path(Path),
}

impl Hash for NameRef {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match self {
            Self::Literal { ref value, .. } => value.hash(h),
            Self::Path(ref path) => path.hash(h),
        }
    }
}

impl PartialEq for NameRef {
    fn eq(&self, other: &NameRef) -> bool {
        match self {
            Self::Literal {
                value: ref my_value,
                ..
            } => match other {
                Self::Literal {
                    value: ref other_value,
                    ..
                } => my_value == other_value,
                _ => false,
            },
            Self::Path(ref my_path) => match other {
                Self::Path(ref other_path) => my_path == other_path,
                _ => false,
            },
        }
    }
}

impl Eq for NameRef {}

impl syn::parse::Parse for NameRef {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        if input.peek(syn::LitStr) {
            let s: LitStr = input.parse()?;
            let span = s.span();
            match NcName::try_from(s.value()) {
                Ok(value) => Ok(Self::Literal { value, span }),
                Err(e) => Err(Error::new(span, format!("not a valid XML name: {}", e))),
            }
        } else {
            let p: Path = input.parse()?;
            Ok(Self::Path(p))
        }
    }
}

impl quote::ToTokens for NameRef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Literal { ref value, span } => {
                let span = *span;
                let value = value.as_str();
                let value = quote_spanned! { span=> #value };
                // SAFETY: self.0 is a known-good NcName, so converting it to an
                // NcNameStr is known to be safe.
                // NOTE: we cannot use `quote_spanned! { self.span=> }` for the unsafe
                // block as that would then in fact trip a `#[deny(unsafe_code)]` lint
                // at the use site of the macro.
                tokens.extend(quote! {
                    unsafe { ::xso::exports::rxml::NcNameStr::from_str_unchecked(#value) }
                })
            }
            Self::Path(ref path) => path.to_tokens(tokens),
        }
    }
}

/// Represents the amount constraint used with child elements.
///
/// Currently, this only supports "one" (literal `1`) or "any amount" (`..`).
/// In the future, we might want to add support for any range pattern for
/// `usize` and any positive integer literal.
#[derive(Debug)]
pub(crate) enum AmountConstraint {
    /// Equivalent to `1`
    #[allow(dead_code)]
    FixedSingle(Span),

    /// Equivalent to `..`.
    Any(Span),
}

impl syn::parse::Parse for AmountConstraint {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        if input.peek(LitInt) && !input.peek2(token::DotDot) && !input.peek2(token::DotDotEq) {
            let lit: LitInt = input.parse()?;
            let value: usize = lit.base10_parse()?;
            if value == 1 {
                Ok(Self::FixedSingle(lit.span()))
            } else {
                Err(Error::new(lit.span(), "only `1` and `..` are allowed here"))
            }
        } else {
            let p: PatRange = input.parse()?;
            if let Some(attr) = p.attrs.first() {
                return Err(Error::new_spanned(attr, "attributes not allowed here"));
            }
            if let Some(start) = p.start.as_ref() {
                return Err(Error::new_spanned(
                    start,
                    "only full ranges (`..`) are allowed here",
                ));
            }
            if let Some(end) = p.end.as_ref() {
                return Err(Error::new_spanned(
                    end,
                    "only full ranges (`..`) are allowed here",
                ));
            }
            Ok(Self::Any(p.span()))
        }
    }
}

/// Represents a boolean flag from a `#[xml(..)]` attribute meta.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Flag {
    /// The flag is not set.
    Absent,

    /// The flag was set.
    Present(
        /// The span of the syntax element which enabled the flag.
        ///
        /// This is used to generate useful error messages by pointing at the
        /// specific place the flag was activated.
        #[allow(dead_code)]
        Span,
    ),
}

impl Flag {
    /// Return true if the flag is set, false otherwise.
    pub(crate) fn is_set(&self) -> bool {
        match self {
            Self::Absent => false,
            Self::Present(_) => true,
        }
    }

    /// Like `Option::take`, but for flags.
    pub(crate) fn take(&mut self) -> Self {
        let mut result = Flag::Absent;
        core::mem::swap(&mut result, self);
        result
    }
}

impl<T: Spanned> From<T> for Flag {
    fn from(other: T) -> Flag {
        Flag::Present(other.span())
    }
}

/// A pair of `namespace` and `name` keys.
#[derive(Debug, Default)]
pub(crate) struct QNameRef {
    /// The XML namespace supplied.
    pub(crate) namespace: Option<NamespaceRef>,

    /// The XML name supplied.
    pub(crate) name: Option<NameRef>,
}

impl QNameRef {
    /// Attempt to incrementally parse this QNameRef.
    ///
    /// If `meta` contains either `namespace` or `name` keys, they are
    /// processed and either `Ok(None)` or an error is returned.
    ///
    /// If no matching key is found, `Ok(Some(meta))` is returned for further
    /// processing.
    fn parse_incremental_from_meta<'x>(
        &mut self,
        meta: ParseNestedMeta<'x>,
    ) -> Result<Option<ParseNestedMeta<'x>>> {
        if meta.path.is_ident("name") {
            if self.name.is_some() {
                return Err(Error::new_spanned(meta.path, "duplicate `name` key"));
            }
            let value = meta.value()?;
            let name_span = value.span();
            let (new_namespace, new_name) = parse_prefixed_name(value)?;
            if let Some(new_namespace) = new_namespace {
                if let Some(namespace) = self.namespace.as_ref() {
                    let mut error = Error::new(
                        name_span,
                        "cannot combine `namespace` key with prefixed `name`",
                    );
                    error.combine(Error::new_spanned(namespace, "`namespace` was set here"));
                    return Err(error);
                }
                self.namespace = Some(new_namespace);
            }
            self.name = Some(new_name);
            Ok(None)
        } else if meta.path.is_ident("namespace") {
            if self.namespace.is_some() {
                return Err(Error::new_spanned(
                    meta.path,
                    "duplicate `namespace` key or `name` key has prefix",
                ));
            }
            self.namespace = Some(meta.value()?.parse()?);
            Ok(None)
        } else {
            Ok(Some(meta))
        }
    }
}

/// Identifies XML content to discard.
#[derive(Debug)]
pub(crate) enum DiscardSpec {
    /// `#[xml(discard(attribute..))]`
    Attribute {
        /// The span of the nested meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The value assigned to `namespace` and `name` fields inside
        /// `#[xml(discard(attribute(..)))]`, if any.
        qname: QNameRef,
    },

    /// `#[xml(discard(text))]`
    Text {
        /// The span of the nested meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,
    },
}

impl DiscardSpec {
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Attribute { ref span, .. } => *span,
            Self::Text { ref span, .. } => *span,
        }
    }
}

impl TryFrom<XmlFieldMeta> for DiscardSpec {
    type Error = syn::Error;

    fn try_from(other: XmlFieldMeta) -> Result<Self> {
        match other {
            XmlFieldMeta::Attribute {
                span,
                qname,
                default_,
                type_,
                codec,
            } => {
                reject_key!(default_ flag not on "discard specifications" only on "fields");
                reject_key!(type_ not on "discard specifications" only on "fields");
                reject_key!(codec not on "discard specifications" only on "fields");
                Ok(Self::Attribute { span, qname })
            }
            XmlFieldMeta::Text { span, type_, codec } => {
                reject_key!(type_ not on "discard specifications" only on "fields");
                reject_key!(codec not on "discard specifications" only on "fields");
                Ok(Self::Text { span })
            }
            other => Err(Error::new(
                other.span(),
                "cannot discard this kind of child",
            )),
        }
    }
}

/// Contents of an `#[xml(..)]` attribute on a struct, enum variant, or enum.
#[derive(Debug)]
pub(crate) struct XmlCompoundMeta {
    /// The span of the `#[xml(..)]` meta from which this was parsed.
    ///
    /// This is useful for error messages.
    pub(crate) span: Span,

    /// The value assigned to `namespace` and `name` fields inside
    /// `#[xml(..)]`, if any.
    pub(crate) qname: QNameRef,

    /// The debug flag.
    pub(crate) debug: Flag,

    /// The value assigned to `builder` inside `#[xml(..)]`, if any.
    pub(crate) builder: Option<Ident>,

    /// The value assigned to `iterator` inside `#[xml(..)]`, if any.
    pub(crate) iterator: Option<Ident>,

    /// The value assigned to `on_unknown_attribute` inside `#[xml(..)]`, if
    /// any.
    pub(crate) on_unknown_attribute: Option<Ident>,

    /// The value assigned to `on_unknown_child` inside `#[xml(..)]`, if
    /// any.
    pub(crate) on_unknown_child: Option<Ident>,

    /// The exhaustive flag.
    pub(crate) exhaustive: Flag,

    /// The transparent flag.
    pub(crate) transparent: Flag,

    /// Items to discard.
    pub(crate) discard: Vec<DiscardSpec>,

    /// The value assigned to `deserialize_callback` inside `#[xml(..)]`, if any.
    pub(crate) deserialize_callback: Option<Path>,
}

impl XmlCompoundMeta {
    /// Parse the meta values from a `#[xml(..)]` attribute.
    ///
    /// Undefined options or options with incompatible values are rejected
    /// with an appropriate compile-time error.
    fn parse_from_attribute(attr: &Attribute) -> Result<Self> {
        let mut qname = QNameRef::default();
        let mut builder = None;
        let mut iterator = None;
        let mut on_unknown_attribute = None;
        let mut on_unknown_child = None;
        let mut debug = Flag::Absent;
        let mut exhaustive = Flag::Absent;
        let mut transparent = Flag::Absent;
        let mut discard = Vec::new();
        let mut deserialize_callback = None;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("debug") {
                if debug.is_set() {
                    return Err(Error::new_spanned(meta.path, "duplicate `debug` key"));
                }
                debug = (&meta.path).into();
                Ok(())
            } else if meta.path.is_ident("builder") {
                if builder.is_some() {
                    return Err(Error::new_spanned(meta.path, "duplicate `builder` key"));
                }
                builder = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("iterator") {
                if iterator.is_some() {
                    return Err(Error::new_spanned(meta.path, "duplicate `iterator` key"));
                }
                iterator = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("on_unknown_attribute") {
                if on_unknown_attribute.is_some() {
                    return Err(Error::new_spanned(
                        meta.path,
                        "duplicate `on_unknown_attribute` key",
                    ));
                }
                on_unknown_attribute = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("on_unknown_child") {
                if on_unknown_child.is_some() {
                    return Err(Error::new_spanned(
                        meta.path,
                        "duplicate `on_unknown_child` key",
                    ));
                }
                on_unknown_child = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("exhaustive") {
                if exhaustive.is_set() {
                    return Err(Error::new_spanned(meta.path, "duplicate `exhaustive` key"));
                }
                exhaustive = (&meta.path).into();
                Ok(())
            } else if meta.path.is_ident("transparent") {
                if transparent.is_set() {
                    return Err(Error::new_spanned(meta.path, "duplicate `transparent` key"));
                }
                transparent = (&meta.path).into();
                Ok(())
            } else if meta.path.is_ident("discard") {
                meta.parse_nested_meta(|meta| {
                    discard.push(XmlFieldMeta::parse_from_meta(meta)?.try_into()?);
                    Ok(())
                })?;
                Ok(())
            } else if meta.path.is_ident("deserialize_callback") {
                if deserialize_callback.is_some() {
                    return Err(Error::new_spanned(
                        meta.path,
                        "duplicate `deserialize_callback` key",
                    ));
                }
                deserialize_callback = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                match qname.parse_incremental_from_meta(meta)? {
                    None => Ok(()),
                    Some(meta) => Err(Error::new_spanned(meta.path, "unsupported key")),
                }
            }
        })?;

        Ok(Self {
            span: attr.span(),
            qname,
            debug,
            builder,
            iterator,
            on_unknown_attribute,
            on_unknown_child,
            exhaustive,
            transparent,
            discard,
            deserialize_callback,
        })
    }

    /// Search through `attrs` for a single `#[xml(..)]` attribute and parse
    /// it.
    ///
    /// Undefined options or options with incompatible values are rejected
    /// with an appropriate compile-time error.
    ///
    /// If more than one `#[xml(..)]` attribute is found, an error is
    /// emitted.
    ///
    /// If no `#[xml(..)]` attribute is found, `None` is returned.
    pub(crate) fn try_parse_from_attributes(attrs: &[Attribute]) -> Result<Option<Self>> {
        let mut result = None;
        for attr in attrs {
            if !attr.path().is_ident("xml") {
                continue;
            }
            if result.is_some() {
                return Err(syn::Error::new_spanned(
                    attr.path(),
                    "only one #[xml(..)] per struct or enum variant allowed",
                ));
            }
            result = Some(Self::parse_from_attribute(attr)?);
        }
        Ok(result)
    }

    /// Search through `attrs` for a single `#[xml(..)]` attribute and parse
    /// it.
    ///
    /// Undefined options or options with incompatible values are rejected
    /// with an appropriate compile-time error.
    ///
    /// If more than one or no `#[xml(..)]` attribute is found, an error is
    /// emitted.
    pub(crate) fn parse_from_attributes(attrs: &[Attribute]) -> Result<Self> {
        match Self::try_parse_from_attributes(attrs)? {
            Some(v) => Ok(v),
            None => Err(syn::Error::new(
                Span::call_site(),
                "#[xml(..)] attribute required on struct or enum variant",
            )),
        }
    }
}

/// Return true if the tokens the cursor points at are a valid type path
/// prefix.
///
/// This does not advance the parse stream.
///
/// If the tokens *do* look like a type path, a Span which points at the first
/// `<` encountered is returned. This can be used for a helpful error message
/// in case parsing the type path does then fail.
fn maybe_type_path(p: parse::ParseStream<'_>) -> (bool, Option<Span>) {
    // ParseStream cursors do not advance the stream, but they are also rather
    // unwieldy to use. Prepare for a lot of `let .. = ..`.

    let cursor = if p.peek(token::PathSep) {
        // If we have a path separator, we need to skip that initially. We
        // do this by skipping two punctuations. We use unwrap() here because
        // we already know for sure that we see two punctuation items (because
        // of the peek).
        p.cursor().punct().unwrap().1.punct().unwrap().1
    } else {
        // No `::` initially, so we just take what we have.
        p.cursor()
    };

    // Now we loop over `$ident::` segments. If we find anything but a `:`
    // after the ident, we exit. Depending on *what* we find, we either exit
    // true or false, but see for yourself.
    let mut cursor = cursor;
    loop {
        // Here we look for the identifier, but we do not care for its
        // contents.
        let Some((_, new_cursor)) = cursor.ident() else {
            return (false, None);
        };
        cursor = new_cursor;

        // Now we see what actually follows the ident (it must be punctuation
        // for it to be a type path...)
        let Some((punct, new_cursor)) = cursor.punct() else {
            return (false, None);
        };
        cursor = new_cursor;

        match punct.as_char() {
            // Looks like a `foo<..`, we treat that as a type path for the
            // reasons stated in [`parse_codec_expr`]'s doc.
            '<' => return (true, Some(punct.span())),

            // Continue looking ahead: looks like a path separator.
            ':' => (),

            // Anything else (such as `,` (separating another argument most
            // likely), or `.` (a method call?)) we treat as "not a type
            // path".
            _ => return (false, None),
        }

        // If we are here, we saw a `:`. Look for the second one.
        let Some((punct, new_cursor)) = cursor.punct() else {
            return (false, None);
        };
        cursor = new_cursor;

        if punct.as_char() != ':' {
            // If it is not another `:`, it cannot be a type path.
            return (false, None);
        }

        // And round and round and round it goes.
        // We will terminate eventually because the cursor will return None
        // on any of the lookups because parse streams are (hopefully!)
        // finite. Most likely, we'll however encounter a `<` or other non-`:`
        // punctuation first.
    }
}

/// Parse expressions passed to `codec`.
///
/// Those will generally be paths to unit type constructors (such as `Foo`)
/// or references to static values or chains of function calls.
///
/// In the case of unit type constructors for generic types, users may type
/// for example `FixedHex<20>`, thinking they are writing a type path. However,
/// while `FixedHex<20>` is indeed a valid type path, it is not a valid
/// expression for a unit type constructor. Instead it is parsed as
/// `FixedHex < 20` and then a syntax error.
///
/// We however know that `Foo < Bar` is never a valid expression for a type.
/// Thus, we can be smart about this and inject the `::` at the right place
/// automatically.
fn parse_codec_expr(p: parse::ParseStream<'_>) -> Result<(Expr, Option<Error>)> {
    let (maybe_type_path, punct_span) = maybe_type_path(p);
    if maybe_type_path {
        let helpful_error =
            punct_span.map(|span| Error::new(span, "help: try inserting a `::` before this `<`"));
        let mut type_path: TypePath = match p.parse() {
            Ok(v) => v,
            Err(mut e) => match helpful_error {
                Some(help) => {
                    e.combine(help);
                    return Err(e);
                }
                None => return Err(e),
            },
        };
        // We got a type path -- so we now inject the `::` before any `<` as
        // needed.
        for segment in type_path.path.segments.iter_mut() {
            if let PathArguments::AngleBracketed(ref mut arguments) = segment.arguments {
                let span = arguments.span();
                arguments.colon2_token.get_or_insert(token::PathSep {
                    spans: [span, span],
                });
            }
        }
        Ok((
            Expr::Path(ExprPath {
                attrs: Vec::new(),
                qself: type_path.qself,
                path: type_path.path,
            }),
            helpful_error,
        ))
    } else {
        p.parse().map(|x| (x, None))
    }
}

/// Parse an XML name while resolving built-in namespace prefixes.
fn parse_prefixed_name(
    value: syn::parse::ParseStream<'_>,
) -> Result<(Option<NamespaceRef>, NameRef)> {
    if !value.peek(LitStr) {
        // if we don't have a string literal next, we delegate to the default
        // `NameRef` parser.
        return Ok((None, value.parse()?));
    }

    let name: LitStr = value.parse()?;
    let name_span = name.span();
    let (prefix, name) = match name
        .value()
        .try_into()
        .and_then(|name: rxml_validation::Name| name.split_name())
    {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::new(
                name_span,
                format!("not a valid XML name: {}", e),
            ))
        }
    };
    let name = NameRef::Literal {
        value: name,
        span: name_span,
    };
    if let Some(prefix) = prefix {
        let namespace_uri = match prefix.as_str() {
            "xml" => XMLNS_XML,
            "xmlns" => XMLNS_XMLNS,
            other => return Err(Error::new(
                name_span,
                format!("prefix `{}` is not a built-in prefix and cannot be used. specify the desired namespace using the `namespace` key instead.", other)
            )),
        };
        Ok((Some(NamespaceRef::fudge(namespace_uri, name_span)), name))
    } else {
        Ok((None, name))
    }
}

/// Contents of an `#[xml(..)]` attribute on a struct or enum variant member.
#[derive(Debug)]
pub(crate) enum XmlFieldMeta {
    /// `#[xml(attribute)]`, `#[xml(attribute = ..)]` or `#[xml(attribute(..))]`
    Attribute {
        /// The span of the `#[xml(attribute)]` meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The namespace/name keys.
        qname: QNameRef,

        /// The `default` flag.
        default_: Flag,

        /// An explicit type override, only usable within extracts.
        type_: Option<Type>,

        /// The path to the optional codec type.
        codec: Option<Expr>,
    },

    /// `#[xml(text)]`
    Text {
        /// The span of the `#[xml(text)]` meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The path to the optional codec type.
        codec: Option<Expr>,

        /// An explicit type override, only usable within extracts.
        type_: Option<Type>,
    },

    /// `#[xml(child)`
    Child {
        /// The span of the `#[xml(child)]` meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The `default` flag.
        default_: Flag,

        /// The `n` flag.
        amount: Option<AmountConstraint>,
    },

    /// `#[xml(extract)]
    Extract {
        /// The span of the `#[xml(extract)]` meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The namespace/name keys.
        qname: QNameRef,

        /// The `n` flag.
        amount: Option<AmountConstraint>,

        /// The `default` flag.
        default_: Flag,

        /// The `fields` nested meta.
        fields: Vec<XmlFieldMeta>,

        /// The `on_unknown_attribute` value.
        on_unknown_attribute: Option<Ident>,

        /// The `on_unknown_child` value.
        on_unknown_child: Option<Ident>,
    },

    /// `#[xml(element)]`
    Element {
        /// The span of the `#[xml(element)]` meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The `default` flag.
        default_: Flag,

        /// The `n` flag.
        amount: Option<AmountConstraint>,
    },

    /// `#[xml(flag)]
    Flag {
        /// The span of the `#[xml(flag)]` meta from which this was parsed.
        ///
        /// This is useful for error messages.
        span: Span,

        /// The namespace/name keys.
        qname: QNameRef,
    },
}

impl XmlFieldMeta {
    /// Parse a `#[xml(attribute(..))]` meta.
    ///
    /// That meta can have three distinct syntax styles:
    /// - argument-less: `#[xml(attribute)]`
    /// - shorthand: `#[xml(attribute = ..)]`
    /// - full: `#[xml(attribute(..))]`
    fn attribute_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        if meta.input.peek(Token![=]) {
            // shorthand syntax
            let (namespace, name) = parse_prefixed_name(meta.value()?)?;
            Ok(Self::Attribute {
                span: meta.path.span(),
                qname: QNameRef {
                    name: Some(name),
                    namespace,
                },
                default_: Flag::Absent,
                type_: None,
                codec: None,
            })
        } else if meta.input.peek(syn::token::Paren) {
            // full syntax
            let mut qname = QNameRef::default();
            let mut default_ = Flag::Absent;
            let mut type_ = None;
            let mut codec = None;
            meta.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    if default_.is_set() {
                        return Err(Error::new_spanned(meta.path, "duplicate `default` key"));
                    }
                    default_ = (&meta.path).into();
                    Ok(())
                } else if meta.path.is_ident("type_") {
                    if type_.is_some() {
                        return Err(Error::new_spanned(meta.path, "duplicate `type_` key"));
                    }
                    type_ = Some(meta.value()?.parse()?);
                    Ok(())
                } else if meta.path.is_ident("codec") {
                    if codec.is_some() {
                        return Err(Error::new_spanned(meta.path, "duplicate `codec` key"));
                    }
                    let (new_codec, helpful_error) = parse_codec_expr(meta.value()?)?;
                    // See the comment at the top of text_from_meta() below for why we
                    // do this.
                    let lookahead = meta.input.lookahead1();
                    if !lookahead.peek(Token![,]) && !meta.input.is_empty() {
                        if let Some(helpful_error) = helpful_error {
                            let mut e = lookahead.error();
                            e.combine(helpful_error);
                            return Err(e);
                        }
                    }
                    codec = Some(new_codec);
                    Ok(())
                } else {
                    match qname.parse_incremental_from_meta(meta)? {
                        None => Ok(()),
                        Some(meta) => Err(Error::new_spanned(meta.path, "unsupported key")),
                    }
                }
            })?;
            Ok(Self::Attribute {
                span: meta.path.span(),
                qname,
                default_,
                type_,
                codec,
            })
        } else {
            // argument-less syntax
            Ok(Self::Attribute {
                span: meta.path.span(),
                qname: QNameRef::default(),
                default_: Flag::Absent,
                type_: None,
                codec: None,
            })
        }
    }

    /// Parse a `#[xml(text)]` meta.
    fn text_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        if meta.input.peek(Token![=]) {
            let (codec, helpful_error) = parse_codec_expr(meta.value()?)?;
            // A meta value can only be followed by either a `,`, or the end
            // of the parse stream (because of the delimited group ending).
            // Hence we check we are there. And if we are *not* there, we emit
            // an error straight away, with the helpful addition from the
            // `parse_codec_expr` if we have it.
            //
            // If we do not do this, the user gets a rather confusing
            // "expected `,`" message if the `maybe_type_path` guess was
            // wrong.
            let lookahead = meta.input.lookahead1();
            if !lookahead.peek(Token![,]) && !meta.input.is_empty() {
                if let Some(helpful_error) = helpful_error {
                    let mut e = lookahead.error();
                    e.combine(helpful_error);
                    return Err(e);
                }
            }
            Ok(Self::Text {
                span: meta.path.span(),
                type_: None,
                codec: Some(codec),
            })
        } else if meta.input.peek(syn::token::Paren) {
            let mut codec: Option<Expr> = None;
            let mut type_: Option<Type> = None;
            meta.parse_nested_meta(|meta| {
                if meta.path.is_ident("codec") {
                    if codec.is_some() {
                        return Err(Error::new_spanned(meta.path, "duplicate `codec` key"));
                    }
                    let (new_codec, helpful_error) = parse_codec_expr(meta.value()?)?;
                    // See above (at the top-ish of this function) for why we
                    // do this.
                    let lookahead = meta.input.lookahead1();
                    if !lookahead.peek(Token![,]) && !meta.input.is_empty() {
                        if let Some(helpful_error) = helpful_error {
                            let mut e = lookahead.error();
                            e.combine(helpful_error);
                            return Err(e);
                        }
                    }
                    codec = Some(new_codec);
                    Ok(())
                } else if meta.path.is_ident("type_") {
                    if type_.is_some() {
                        return Err(Error::new_spanned(meta.path, "duplicate `type_` key"));
                    }
                    type_ = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(Error::new_spanned(meta.path, "unsupported key"))
                }
            })?;
            Ok(Self::Text {
                span: meta.path.span(),
                type_,
                codec,
            })
        } else {
            Ok(Self::Text {
                span: meta.path.span(),
                type_: None,
                codec: None,
            })
        }
    }

    /// Parse a `#[xml(child)]` meta.
    fn child_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        if meta.input.peek(syn::token::Paren) {
            let mut default_ = Flag::Absent;
            let mut amount = None;
            meta.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    if default_.is_set() {
                        return Err(Error::new_spanned(meta.path, "duplicate `default` key"));
                    }
                    default_ = (&meta.path).into();
                    Ok(())
                } else if meta.path.is_ident("n") {
                    if amount.is_some() {
                        return Err(Error::new_spanned(meta.path, "duplicate `n` key"));
                    }
                    amount = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(Error::new_spanned(meta.path, "unsupported key"))
                }
            })?;
            Ok(Self::Child {
                span: meta.path.span(),
                default_,
                amount,
            })
        } else {
            Ok(Self::Child {
                span: meta.path.span(),
                default_: Flag::Absent,
                amount: None,
            })
        }
    }

    /// Parse a `#[xml(extract)]` meta.
    fn extract_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        let mut qname = QNameRef::default();
        let mut fields = None;
        let mut amount = None;
        let mut on_unknown_attribute = None;
        let mut on_unknown_child = None;
        let mut default_ = Flag::Absent;
        meta.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                if default_.is_set() {
                    return Err(Error::new_spanned(meta.path, "duplicate `default` key"));
                }
                default_ = (&meta.path).into();
                Ok(())
            } else if meta.path.is_ident("fields") {
                if let Some((fields_span, _)) = fields.as_ref() {
                    let mut error = Error::new_spanned(meta.path, "duplicate `fields` meta");
                    error.combine(Error::new(*fields_span, "previous `fields` meta was here"));
                    return Err(error);
                }
                let mut new_fields = Vec::new();
                meta.parse_nested_meta(|meta| {
                    new_fields.push(XmlFieldMeta::parse_from_meta(meta)?);
                    Ok(())
                })?;
                fields = Some((meta.path.span(), new_fields));
                Ok(())
            } else if meta.path.is_ident("n") {
                if amount.is_some() {
                    return Err(Error::new_spanned(meta.path, "duplicate `n` key"));
                }
                amount = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("on_unknown_attribute") {
                if on_unknown_attribute.is_some() {
                    return Err(Error::new_spanned(
                        meta.path,
                        "duplicate `on_unknown_attribute` key",
                    ));
                }
                on_unknown_attribute = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("on_unknown_child") {
                if on_unknown_child.is_some() {
                    return Err(Error::new_spanned(
                        meta.path,
                        "duplicate `on_unknown_child` key",
                    ));
                }
                on_unknown_child = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                match qname.parse_incremental_from_meta(meta)? {
                    None => Ok(()),
                    Some(meta) => Err(Error::new_spanned(meta.path, "unsupported key")),
                }
            }
        })?;
        let fields = fields.map(|(_, x)| x).unwrap_or_else(Vec::new);
        Ok(Self::Extract {
            span: meta.path.span(),
            default_,
            qname,
            fields,
            amount,
            on_unknown_attribute,
            on_unknown_child,
        })
    }

    /// Parse a `#[xml(element)]` meta.
    fn element_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        let mut amount = None;
        let mut default_ = Flag::Absent;
        if meta.input.peek(syn::token::Paren) {
            meta.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    if default_.is_set() {
                        return Err(Error::new_spanned(meta.path, "duplicate `default` key"));
                    }
                    default_ = (&meta.path).into();
                    Ok(())
                } else if meta.path.is_ident("n") {
                    if amount.is_some() {
                        return Err(Error::new_spanned(meta.path, "duplicate `n` key"));
                    }
                    amount = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(Error::new_spanned(meta.path, "unsupported key"))
                }
            })?;
        }
        Ok(Self::Element {
            span: meta.path.span(),
            default_,
            amount,
        })
    }

    /// Parse a `#[xml(flag)]` meta.
    fn flag_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        let mut qname = QNameRef::default();
        if meta.input.peek(syn::token::Paren) {
            meta.parse_nested_meta(|meta| match qname.parse_incremental_from_meta(meta)? {
                None => Ok(()),
                Some(meta) => Err(Error::new_spanned(meta.path, "unsupported key")),
            })?;
        }
        Ok(Self::Flag {
            span: meta.path.span(),
            qname,
        })
    }

    /// Parse [`Self`] from a nestd meta, switching on the identifier
    /// of that nested meta.
    fn parse_from_meta(meta: ParseNestedMeta<'_>) -> Result<Self> {
        if meta.path.is_ident("attribute") {
            Self::attribute_from_meta(meta)
        } else if meta.path.is_ident("text") {
            Self::text_from_meta(meta)
        } else if meta.path.is_ident("child") {
            Self::child_from_meta(meta)
        } else if meta.path.is_ident("extract") {
            Self::extract_from_meta(meta)
        } else if meta.path.is_ident("element") {
            Self::element_from_meta(meta)
        } else if meta.path.is_ident("flag") {
            Self::flag_from_meta(meta)
        } else {
            Err(Error::new_spanned(meta.path, "unsupported field meta"))
        }
    }

    /// Parse an `#[xml(..)]` meta on a field.
    ///
    /// This switches based on the first identifier within the `#[xml(..)]`
    /// meta and generates an enum variant accordingly.
    ///
    /// Only a single nested meta is allowed; more than one will be
    /// rejected with an appropriate compile-time error.
    ///
    /// If no meta is contained at all, a compile-time error is generated.
    ///
    /// Undefined options or options with incompatible values are rejected
    /// with an appropriate compile-time error.
    pub(crate) fn parse_from_attribute(attr: &Attribute) -> Result<Self> {
        let mut result: Option<Self> = None;

        attr.parse_nested_meta(|meta| {
            if result.is_some() {
                return Err(Error::new_spanned(
                    meta.path,
                    "multiple field type specifiers are not supported",
                ));
            }

            result = Some(Self::parse_from_meta(meta)?);
            Ok(())
        })?;

        if let Some(result) = result {
            Ok(result)
        } else {
            Err(Error::new_spanned(
                attr,
                "missing field type specifier within `#[xml(..)]`",
            ))
        }
    }

    /// Find and parse a `#[xml(..)]` meta on a field.
    ///
    /// This invokes [`Self::parse_from_attribute`] internally on the first
    /// encountered `#[xml(..)]` meta.
    ///
    /// If not exactly one `#[xml(..)]` meta is encountered, an error is
    /// returned. The error is spanned to `err_span`.
    pub(crate) fn parse_from_attributes(attrs: &[Attribute], err_span: &Span) -> Result<Self> {
        let mut result: Option<Self> = None;
        for attr in attrs {
            if !attr.path().is_ident("xml") {
                continue;
            }

            if result.is_some() {
                return Err(Error::new_spanned(
                    attr,
                    "only one #[xml(..)] attribute per field allowed.",
                ));
            }

            result = Some(Self::parse_from_attribute(attr)?);
        }

        if let Some(result) = result {
            Ok(result)
        } else {
            Err(Error::new(*err_span, "missing #[xml(..)] meta on field"))
        }
    }

    /// Return a span which points at the meta which constructed this
    /// XmlFieldMeta.
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Attribute { ref span, .. } => *span,
            Self::Child { ref span, .. } => *span,
            Self::Text { ref span, .. } => *span,
            Self::Extract { ref span, .. } => *span,
            Self::Element { ref span, .. } => *span,
            Self::Flag { ref span, .. } => *span,
        }
    }

    /// Extract an explicit type specification if it exists.
    pub(crate) fn take_type(&mut self) -> Option<Type> {
        match self {
            Self::Attribute { ref mut type_, .. } => type_.take(),
            Self::Text { ref mut type_, .. } => type_.take(),
            _ => None,
        }
    }
}
