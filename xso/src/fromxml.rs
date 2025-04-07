//! # Generic builder type implementations
//!
//! This module contains [`FromEventsBuilder`] implementations for types from
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

use crate::error::{Error, FromEventsError};
use crate::{FromEventsBuilder, FromXml};

/// Helper struct to construct an `Option<T>` from XML events.
pub struct OptionBuilder<T: FromEventsBuilder>(T);

impl<T: FromEventsBuilder> FromEventsBuilder for OptionBuilder<T> {
    type Output = Option<T::Output>;

    fn feed(&mut self, ev: rxml::Event) -> Result<Option<Self::Output>, Error> {
        self.0.feed(ev).map(|ok| ok.map(Some))
    }
}

/// Parsers `T` into `Some(.)`.
///
/// Note that this never generates `None`: The main use case is to allow
/// external (i.e. without calling `from_events`) defaulting to `None` and
/// for optional serialisation (the [`AsXml`][`crate::AsXml`] implementation
/// on `Option<T>` emits nothing for `None`).
impl<T: FromXml> FromXml for Option<T> {
    type Builder = OptionBuilder<T::Builder>;

    fn from_events(
        name: rxml::QName,
        attrs: rxml::AttrMap,
    ) -> Result<Self::Builder, FromEventsError> {
        Ok(OptionBuilder(T::from_events(name, attrs)?))
    }
}

/// Helper struct to construct an `Box<T>` from XML events.
pub struct BoxBuilder<T: FromEventsBuilder>(Box<T>);

impl<T: FromEventsBuilder> FromEventsBuilder for BoxBuilder<T> {
    type Output = Box<T::Output>;

    fn feed(&mut self, ev: rxml::Event) -> Result<Option<Self::Output>, Error> {
        self.0.feed(ev).map(|ok| ok.map(Box::new))
    }
}

/// Parsers `T` into a `Box`.
impl<T: FromXml> FromXml for Box<T> {
    type Builder = BoxBuilder<T::Builder>;

    fn from_events(
        name: rxml::QName,
        attrs: rxml::AttrMap,
    ) -> Result<Self::Builder, FromEventsError> {
        Ok(BoxBuilder(Box::new(T::from_events(name, attrs)?)))
    }
}

#[derive(Debug)]
enum FallibleBuilderInner<T: FromEventsBuilder, E> {
    Processing { depth: usize, builder: T },
    Failed { depth: usize, err: Option<E> },
    Done,
}

/// Build a `Result<T, E>` from XML.
///
/// This builder, invoked generally via the [`FromXml`] implementation on
/// `Result<T, E> where T: FromXml, E: From<Error>`, allows to fallably parse
/// an XSO from XML.
///
/// If an error occurs while parsing the XSO, the remaining events which
/// belong to that XSO are discarded. Once all events have been seen, the
/// error is returned as `Err(.)` value.
///
/// If parsing succeeds, the parsed XSO is returned as `Ok(.)` value.
#[derive(Debug)]
pub struct FallibleBuilder<T: FromEventsBuilder, E>(FallibleBuilderInner<T, E>);

impl<T: FromEventsBuilder, E: From<Error>> FromEventsBuilder for FallibleBuilder<T, E> {
    type Output = Result<T::Output, E>;

    fn feed(&mut self, ev: rxml::Event) -> Result<Option<Self::Output>, Error> {
        match self.0 {
            FallibleBuilderInner::Processing {
                ref mut depth,
                ref mut builder,
            } => {
                let new_depth = match ev {
                    rxml::Event::StartElement(..) => match depth.checked_add(1) {
                        // I *think* it is OK to return an err here
                        // instead of panicking. The reason is that anyone
                        // who intends to resume processing at the level
                        // of where we started to parse this thing in case
                        // of an error either has to:
                        // - Use this fallible implementation and rely on
                        //   it capturing the error (which we don't in
                        //   this case).
                        // - Or count the depth themselves, which will
                        //   either fail in the same way, or they use a
                        //   wider type (in which case it's ok).
                        None => {
                            self.0 = FallibleBuilderInner::Done;
                            return Err(Error::Other("maximum XML nesting depth exceeded"));
                        }
                        Some(v) => Some(v),
                    },
                    // In case of an element end, underflow means that we
                    // have reached the end of the XSO we wanted to process.
                    // We handle that case at the end of the outer match's
                    // body: Either we have returned a value then (good), or,
                    // if we reach the end there with a new_depth == None,
                    // something went horribly wrong (and we panic).
                    rxml::Event::EndElement(..) => depth.checked_sub(1),

                    // Text and XML declarations have no influence on parsing
                    // depth.
                    rxml::Event::XmlDeclaration(..) | rxml::Event::Text(..) => Some(*depth),
                };

                match builder.feed(ev) {
                    Ok(Some(v)) => {
                        self.0 = FallibleBuilderInner::Done;
                        return Ok(Some(Ok(v)));
                    }
                    Ok(None) => {
                        // continue processing in the next round.
                    }
                    Err(e) => {
                        // We are now officially failed ..
                        match new_depth {
                            // .. but we are not done yet, so enter the
                            // failure backtracking state.
                            Some(depth) => {
                                self.0 = FallibleBuilderInner::Failed {
                                    depth,
                                    err: Some(e.into()),
                                };
                                return Ok(None);
                            }
                            // .. and we are done with parsing, so we return
                            // the error as value.
                            None => {
                                self.0 = FallibleBuilderInner::Done;
                                return Ok(Some(Err(e.into())));
                            }
                        }
                    }
                };

                *depth = match new_depth {
                    Some(v) => v,
                    None => unreachable!("fallible parsing continued beyond end of element"),
                };

                // Need more events.
                Ok(None)
            }
            FallibleBuilderInner::Failed {
                ref mut depth,
                ref mut err,
            } => {
                *depth = match ev {
                    rxml::Event::StartElement(..) => match depth.checked_add(1) {
                        // See above for error return rationale.
                        None => {
                            self.0 = FallibleBuilderInner::Done;
                            return Err(Error::Other("maximum XML nesting depth exceeded"));
                        }
                        Some(v) => v,
                    },
                    rxml::Event::EndElement(..) => match depth.checked_sub(1) {
                        Some(v) => v,
                        None => {
                            // We are officially done, return a value, switch
                            // states, and be done with it.
                            let err = err.take().expect("fallible parsing somehow lost its error");
                            self.0 = FallibleBuilderInner::Done;
                            return Ok(Some(Err(err)));
                        }
                    },

                    // Text and XML declarations have no influence on parsing
                    // depth.
                    rxml::Event::XmlDeclaration(..) | rxml::Event::Text(..) => *depth,
                };

                // Need more events
                Ok(None)
            }
            FallibleBuilderInner::Done => {
                panic!("FromEventsBuilder called after it returned a value")
            }
        }
    }
}

/// Parsers `T` fallibly. See [`FallibleBuilder`] for details.
impl<T: FromXml, E: From<Error>> FromXml for Result<T, E> {
    type Builder = FallibleBuilder<T::Builder, E>;

    fn from_events(
        name: rxml::QName,
        attrs: rxml::AttrMap,
    ) -> Result<Self::Builder, FromEventsError> {
        match T::from_events(name, attrs) {
            Ok(builder) => Ok(FallibleBuilder(FallibleBuilderInner::Processing {
                depth: 0,
                builder,
            })),
            Err(FromEventsError::Mismatch { name, attrs }) => {
                Err(FromEventsError::Mismatch { name, attrs })
            }
            Err(FromEventsError::Invalid(e)) => Ok(FallibleBuilder(FallibleBuilderInner::Failed {
                depth: 0,
                err: Some(e.into()),
            })),
        }
    }
}

/// Builder which discards an entire child tree without inspecting the
/// contents.
#[derive(Debug, Default)]
pub struct Discard {
    depth: usize,
}

impl Discard {
    /// Create a new discarding builder.
    pub fn new() -> Self {
        Self::default()
    }
}

impl FromEventsBuilder for Discard {
    type Output = ();

    fn feed(&mut self, ev: rxml::Event) -> Result<Option<Self::Output>, Error> {
        match ev {
            rxml::Event::StartElement(..) => {
                self.depth = match self.depth.checked_add(1) {
                    Some(v) => v,
                    None => return Err(Error::Other("maximum XML nesting depth exceeded")),
                };
                Ok(None)
            }
            rxml::Event::EndElement(..) => match self.depth.checked_sub(1) {
                None => Ok(Some(())),
                Some(v) => {
                    self.depth = v;
                    Ok(None)
                }
            },
            _ => Ok(None),
        }
    }
}

/// Builder which discards the contents (or raises on unexpected contents).
///
/// This builder is only to be used from within the proc macros and is not
/// stable, public API.
#[doc(hidden)]
#[cfg(feature = "macros")]
pub struct EmptyBuilder {
    childerr: &'static str,
    texterr: &'static str,
}

#[cfg(feature = "macros")]
impl FromEventsBuilder for EmptyBuilder {
    type Output = ();

    fn feed(&mut self, ev: rxml::Event) -> Result<Option<Self::Output>, Error> {
        match ev {
            rxml::Event::EndElement(..) => Ok(Some(())),
            rxml::Event::StartElement(..) => Err(Error::Other(self.childerr)),
            rxml::Event::Text(..) => Err(Error::Other(self.texterr)),
            _ => Err(Error::Other(
                "unexpected content in supposed-to-be-empty element",
            )),
        }
    }
}

/// Precursor struct for [`EmptyBuilder`].
///
/// This struct is only to be used from within the proc macros and is not
/// stable, public API.
#[doc(hidden)]
#[cfg(feature = "macros")]
pub struct Empty {
    pub attributeerr: &'static str,
    pub childerr: &'static str,
    pub texterr: &'static str,
}

#[cfg(feature = "macros")]
impl Empty {
    pub fn start(self, attr: rxml::AttrMap) -> Result<EmptyBuilder, Error> {
        if !attr.is_empty() {
            return Err(Error::Other(self.attributeerr));
        }
        Ok(EmptyBuilder {
            childerr: self.childerr,
            texterr: self.texterr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::borrow::ToOwned;
    use rxml::{parser::EventMetrics, Event, Namespace, NcName};

    macro_rules! null_builder {
        ($name:ident for $output:ident) => {
            #[derive(Debug)]
            enum $name {}

            impl FromEventsBuilder for $name {
                type Output = $output;

                fn feed(&mut self, _: Event) -> Result<Option<Self::Output>, Error> {
                    unreachable!();
                }
            }
        };
    }

    null_builder!(AlwaysMismatchBuilder for AlwaysMismatch);
    null_builder!(InitialErrorBuilder for InitialError);

    #[derive(Debug)]
    struct AlwaysMismatch;

    impl FromXml for AlwaysMismatch {
        type Builder = AlwaysMismatchBuilder;

        fn from_events(
            name: rxml::QName,
            attrs: rxml::AttrMap,
        ) -> Result<Self::Builder, FromEventsError> {
            Err(FromEventsError::Mismatch { name, attrs })
        }
    }

    #[derive(Debug)]
    struct InitialError;

    impl FromXml for InitialError {
        type Builder = InitialErrorBuilder;

        fn from_events(_: rxml::QName, _: rxml::AttrMap) -> Result<Self::Builder, FromEventsError> {
            Err(FromEventsError::Invalid(Error::Other("some error")))
        }
    }

    #[derive(Debug)]
    struct FailOnContentBuilder;

    impl FromEventsBuilder for FailOnContentBuilder {
        type Output = FailOnContent;

        fn feed(&mut self, _: Event) -> Result<Option<Self::Output>, Error> {
            Err(Error::Other("content error"))
        }
    }

    #[derive(Debug)]
    struct FailOnContent;

    impl FromXml for FailOnContent {
        type Builder = FailOnContentBuilder;

        fn from_events(_: rxml::QName, _: rxml::AttrMap) -> Result<Self::Builder, FromEventsError> {
            Ok(FailOnContentBuilder)
        }
    }

    fn qname() -> rxml::QName {
        (Namespace::NONE, NcName::try_from("test").unwrap())
    }

    fn attrs() -> rxml::AttrMap {
        rxml::AttrMap::new()
    }

    #[test]
    fn fallible_builder_mismatch_passthrough() {
        match Result::<AlwaysMismatch, Error>::from_events(qname(), attrs()) {
            Err(FromEventsError::Mismatch { .. }) => (),
            other => panic!("unexpected result: {:?}", other),
        }
    }

    #[test]
    fn fallible_builder_initial_error_capture() {
        let mut builder = match Result::<InitialError, Error>::from_events(qname(), attrs()) {
            Ok(v) => v,
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(Some(Err(Error::Other("some error")))) => (),
            other => panic!("unexpected result: {:?}", other),
        };
    }

    #[test]
    fn fallible_builder_initial_error_capture_allows_nested_stuff() {
        let mut builder = match Result::<InitialError, Error>::from_events(qname(), attrs()) {
            Ok(v) => v,
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::StartElement(EventMetrics::zero(), qname(), attrs())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::StartElement(EventMetrics::zero(), qname(), attrs())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::StartElement(EventMetrics::zero(), qname(), attrs())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(Some(Err(Error::Other("some error")))) => (),
            other => panic!("unexpected result: {:?}", other),
        };
    }

    #[test]
    fn fallible_builder_content_error_capture() {
        let mut builder = match Result::<FailOnContent, Error>::from_events(qname(), attrs()) {
            Ok(v) => v,
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(Some(Err(Error::Other("content error")))) => (),
            other => panic!("unexpected result: {:?}", other),
        };
    }

    #[test]
    fn fallible_builder_content_error_capture_with_more_content() {
        let mut builder = match Result::<FailOnContent, Error>::from_events(qname(), attrs()) {
            Ok(v) => v,
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(Some(Err(Error::Other("content error")))) => (),
            other => panic!("unexpected result: {:?}", other),
        };
    }

    #[test]
    fn fallible_builder_content_error_capture_with_nested_content() {
        let mut builder = match Result::<FailOnContent, Error>::from_events(qname(), attrs()) {
            Ok(v) => v,
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::StartElement(EventMetrics::zero(), qname(), attrs())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::StartElement(EventMetrics::zero(), qname(), attrs())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::StartElement(EventMetrics::zero(), qname(), attrs())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::Text(EventMetrics::zero(), "hello world!".to_owned())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(None) => (),
            other => panic!("unexpected result: {:?}", other),
        };
        match builder.feed(Event::EndElement(EventMetrics::zero())) {
            Ok(Some(Err(Error::Other("content error")))) => (),
            other => panic!("unexpected result: {:?}", other),
        };
    }
}
