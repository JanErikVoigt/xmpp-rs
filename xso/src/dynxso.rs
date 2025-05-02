// Copyright (c) 2025 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! # Dynamically-typed XSOs
//!
//! This module provides the utilities to make dynamically-typed XSOs work.
//! Dynamically typed XSOs are contained in [`Xso<dyn Trait>`][`Xso`], where
//! `Trait` is a trait provided by the user.
//!
//! The given `Trait` constrains the specific types which can be used in the
//! `Xso<dyn Trait>` box. This allows users to provide additional methods on
//! the trait which are available on all `Xso<dyn Trait>` objects via the
//! [`Deref`][`core::ops::Deref`] and [`DerefMut`][`core::ops::DerefMut`]
//! implementations.
//!
//! ## Creating a new `Trait`
//!
//! In order to be usable within `Xso<dyn Trait>`, a trait must satisfy the
//! following constraints:
//!
//! - `dyn Trait` must implement [`DynXso`].
//! - `dyn Trait` must implement [`MayContain<T>`] for all `T` which implement
//!   `Trait`.
//!
//! The easiest and most forward-compatible way of providing these
//! implementations is the [`derive_dyn_traits`][`crate::derive_dyn_traits`]
//! macro.
//!
//! ## Example
//!
#![cfg_attr(
    not(feature = "macros"),
    doc = "Because the macros feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
)]
#![cfg_attr(feature = "macros", doc = "\n```\n")]
//! # use core::any::Any;
//! # use xso::{dynxso::{Xso, BuilderRegistry}, FromXml, from_bytes, derive_dyn_traits};
//! trait MyPayload: Any {}
//!
//! derive_dyn_traits!(MyPayload);
//!
//! #[derive(FromXml, Debug, PartialEq)]
//! #[xml(namespace = "urn:example", name = "foo")]
//! struct Foo;
//! impl MyPayload for Foo {}
//! Xso::<dyn MyPayload>::register_type::<Foo>();
//!
//! #[derive(FromXml, Debug, PartialEq)]
//! #[xml(namespace = "urn:example", name = "bar")]
//! struct Bar;
//! impl MyPayload for Bar {}
//! Xso::<dyn MyPayload>::register_type::<Bar>();
//!
//! let x: Xso<dyn MyPayload> = from_bytes("<foo xmlns='urn:example'/>".as_bytes()).unwrap();
//! assert_eq!(Foo, *x.downcast::<Foo>().unwrap());
//!
//! let x: Xso<dyn MyPayload> = from_bytes("<bar xmlns='urn:example'/>".as_bytes()).unwrap();
//! assert_eq!(Bar, *x.downcast::<Bar>().unwrap());
//! ```

use alloc::{boxed::Box, vec::Vec};
use core::{
    any::{Any, TypeId},
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};
use std::sync::Mutex;

use crate::{
    asxml::AsXmlDyn,
    error::{Error, FromEventsError},
    fromxml::XmlNameMatcher,
    AsXml, Context, FromEventsBuilder, FromXml, Item,
};

/// # Generate `DynXso` and `MayContain` trait implementations
///
/// This macro generates trait [`DynXso`] and [`MayContain`] trait
/// implementations for a given trait. For more background information on when
/// that is a useful thing to have, see the [`dynxso`][`crate::dynxso`]
/// module.
///
/// ## Example
///
/// ```
/// # use core::any::Any;
/// use xso::derive_dyn_traits;
/// trait Foo: Any {}
/// derive_dyn_traits!(Foo);
/// ```
///
/// Note that the trait this macro is called on **must** have a bound on
/// `Any`, otherwise the generated code will not compile:
///
/// ```compile_fail
/// use xso::derive_dyn_traits;
/// trait Foo {}
/// derive_dyn_traits!(Foo);
/// // ↑ will generate a bunch of errors about incompatible types
/// ```
#[macro_export]
macro_rules! derive_dyn_traits {
    ($trait:ident) => {
        impl $crate::dynxso::DynXso for dyn $trait {
            fn registry() -> &'static $crate::dynxso::BuilderRegistry<Self> {
                static DATA: $crate::dynxso::BuilderRegistry<dyn $trait> =
                    $crate::dynxso::BuilderRegistry::new();
                &DATA
            }

            fn try_downcast<T: 'static>(
                self: $crate::exports::alloc::boxed::Box<Self>,
            ) -> Result<
                $crate::exports::alloc::boxed::Box<T>,
                $crate::exports::alloc::boxed::Box<Self>,
            >
            where
                Self: $crate::dynxso::MayContain<T>,
            {
                if (&*self as &dyn core::any::Any).is::<T>() {
                    match (self as $crate::exports::alloc::boxed::Box<dyn core::any::Any>)
                        .downcast()
                    {
                        Ok(v) => Ok(v),
                        Err(_) => unreachable!("Any::is and Any::downcast disagree!"),
                    }
                } else {
                    Err(self)
                }
            }

            fn try_downcast_ref<T: 'static>(&self) -> Option<&T>
            where
                Self: $crate::dynxso::MayContain<T>,
            {
                (&*self as &dyn core::any::Any).downcast_ref()
            }

            fn try_downcast_mut<T: 'static>(&mut self) -> Option<&mut T>
            where
                Self: $crate::dynxso::MayContain<T>,
            {
                (&mut *self as &mut dyn core::any::Any).downcast_mut()
            }

            fn is<T: 'static>(&self) -> bool
            where
                Self: $crate::dynxso::MayContain<T>,
            {
                (&*self as &dyn core::any::Any).is::<T>()
            }

            fn type_id(&self) -> core::any::TypeId {
                (&*self as &dyn core::any::Any).type_id()
            }
        }

        impl<T: $trait> $crate::dynxso::MayContain<T> for dyn $trait {
            fn upcast_into(other: T) -> Box<Self> {
                Box::new(other)
            }
        }
    };
}

type BuilderRegistryBuilder<T> = Box<
    dyn Fn(
            rxml::QName,
            rxml::AttrMap,
            &Context<'_>,
        ) -> Result<Box<dyn FromEventsBuilder<Output = Box<T>>>, FromEventsError>
        + Send
        + Sync
        + 'static,
>;

struct BuilderRegistryEntry<T: ?Sized> {
    matcher: XmlNameMatcher<'static>,
    ty: TypeId,
    builder: BuilderRegistryBuilder<T>,
}

impl<T: ?Sized> fmt::Debug for BuilderRegistryEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BuilderRegistryEntry")
            .field("matcher", &self.matcher)
            .field("ty", &self.ty)
            .finish_non_exhaustive()
    }
}

/// # Registry for type-erased [`FromXml`] builders which construct `T`
///
/// This registry holds type-erased [`FromXml::from_events`] implementations.
/// It can be used to dynamically dispatch to a set of `FromXml`
/// implementations which is not known at compile time.
///
/// Under the hood, this is used by the `FromXml` implementation on
/// [`Xso<T>`][`Xso`], via the [`DynXso`] trait.
///
/// Note that this registry does not allow to add arbitrary builders. All
/// builders must originate in a [`FromXml`] implementation and their output
/// must be convertible to `T`.
///
/// ## Performance trade-offs
///
/// The implementation of the `BuilderRegistry` is optimized toward specific
/// usages. Other usages may see negative performance impacts. The following
/// assumptions are made:
///
/// - Types are added only once at startup and a matching
///   [`reserve`][`Self::reserve`] call is made beforehands.
/// - There are many different types.
/// - [`FromXml::xml_name_matcher`] returns a different value for most of the
///   types which are added.
///
/// The lookup algorithms in particular are geared toward the implications of
/// the last two assumptions and may be rather inefficient otherwise.
///
/// ## Example
///
/// This example illustrates the manual usage of the `BuilderRegistry`.
///
#[cfg_attr(
    not(feature = "macros"),
    doc = "Because the macros feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
)]
#[cfg_attr(feature = "macros", doc = "\n```\n")]
/// # use xso::{dynxso::{BuilderRegistry, MayContain}, FromXml, from_bytes, exports::rxml::{Namespace, AttrMap, QName, Event, parser::EventMetrics, xml_ncname}, Context, FromEventsBuilder, error::FromEventsError};
/// #[derive(FromXml, Debug, PartialEq)]
/// #[xml(namespace = "urn:example", name = "foo")]
/// struct Foo;
///
/// #[derive(FromXml, Debug, PartialEq)]
/// #[xml(namespace = "urn:example", name = "bar")]
/// struct Bar;
///
/// #[derive(Debug, PartialEq)]
/// enum Either {
///    A(Foo),
///    B(Bar),
/// }
///
/// // BuilderRegistry::add::<U> requires that Either: MayContain<U>, in order
/// // to be able to wrap the U::Builder such that it outputs a Xso<Either>
/// // instead.
/// impl MayContain<Foo> for Either {
///     fn upcast_into(other: Foo) -> Box<Self> { Box::new(Self::A(other)) }
/// }
///
/// impl MayContain<Bar> for Either {
///     fn upcast_into(other: Bar) -> Box<Self> { Box::new(Self::B(other)) }
/// }
///
/// let registry = BuilderRegistry::<Either>::new();
/// registry.add::<Foo>();
/// registry.add::<Bar>();
///
/// let mut builder = registry.make_builder(
///     (
///         Namespace::from_str("urn:example"),
///         xml_ncname!("foo").to_owned(),   // <- Selects the Foo variant
///     ),
///     AttrMap::new(),
///     &Context::empty(),
/// ).unwrap();
/// let x = builder.feed(Event::EndElement(EventMetrics::zero()), &Context::empty()).unwrap().unwrap();
/// assert_eq!(Either::A(Foo), *x);
///
/// let mut builder = registry.make_builder(
///     (
///         Namespace::from_str("urn:example"),
///         xml_ncname!("bar").to_owned(),   // <- Selects the Bar variant
///     ),
///     AttrMap::new(),
///     &Context::empty(),
/// ).unwrap();
/// let x = builder.feed(Event::EndElement(EventMetrics::zero()), &Context::empty()).unwrap().unwrap();
/// assert_eq!(Either::B(Bar), *x);
/// ```
///
/// Implementing `FromXml` on the `Either` type in the above example would
/// be trivial and look like this:
///
#[cfg_attr(
    not(feature = "macros"),
    doc = "Because the macros feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
)]
#[cfg_attr(feature = "macros", doc = "\n```\n")]
/// # use xso::{dynxso::{BuilderRegistry, MayContain}, FromXml, from_bytes, exports::rxml::{Namespace, AttrMap, QName, Event, parser::EventMetrics}, Context, FromEventsBuilder, error::FromEventsError};
/// # #[derive(FromXml, Debug, PartialEq)]
/// # #[xml(namespace = "urn:example", name = "foo")]
/// # struct Foo;
/// #
/// # #[derive(FromXml, Debug, PartialEq)]
/// # #[xml(namespace = "urn:example", name = "bar")]
/// # struct Bar;
/// #
/// # #[derive(Debug, PartialEq)]
/// # enum Either {
/// #    A(Foo),
/// #    B(Bar),
/// # }
/// #
/// # impl MayContain<Foo> for Either {
/// #     fn upcast_into(other: Foo) -> Box<Self> { Box::new(Self::A(other)) }
/// # }
/// #
/// # impl MayContain<Bar> for Either {
/// #     fn upcast_into(other: Bar) -> Box<Self> { Box::new(Self::B(other)) }
/// # }
/// #
/// // ... code from the previous example ...
/// use xso::dynxso::UnboxBuilder;
/// // In order to be able to use the registry from the FromXml implementation,
/// // it must be a static (as opposed to a local variable as in the previous
/// // example).
/// static REGISTRY: BuilderRegistry<Either> = BuilderRegistry::new();
/// REGISTRY.add::<Foo>();
/// REGISTRY.add::<Bar>();
///
/// impl FromXml for Either {
///     type Builder = UnboxBuilder<Box<dyn FromEventsBuilder<Output = Box<Either>>>>;
///
///     fn from_events(name: QName, attrs: AttrMap, ctx: &Context<'_>) -> Result<Self::Builder, FromEventsError> {
///         REGISTRY.make_builder(name, attrs, ctx).map(UnboxBuilder::wrap)
///     }
/// }
///
/// assert_eq!(
///     Either::A(Foo),
///     from_bytes("<foo xmlns='urn:example'/>".as_bytes()).unwrap(),
/// );
///
/// assert_eq!(
///     Either::B(Bar),
///     from_bytes("<bar xmlns='urn:example'/>".as_bytes()).unwrap(),
/// );
/// ```
#[derive(Debug)]
pub struct BuilderRegistry<T: ?Sized> {
    inner: Mutex<Vec<BuilderRegistryEntry<T>>>,
}

impl<T: ?Sized + 'static> BuilderRegistry<T> {
    /// Create an empty registry.
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
        }
    }

    fn insert(
        &self,
        matcher: XmlNameMatcher<'static>,
        type_id: TypeId,
        builder: BuilderRegistryBuilder<T>,
    ) {
        let mut registry = self.inner.lock().unwrap();
        let start_scan_at = registry.partition_point(|entry| entry.matcher < matcher);
        let insert_at = 'outer: {
            let mut i = start_scan_at;
            while i < registry.len() {
                let entry = &registry[i];
                if entry.matcher == matcher {
                    if entry.ty == type_id {
                        // Already inserted.
                        return;
                    }
                    // Still entries with the same matcher -> continue.
                    i += 1;
                    continue;
                }

                // Found insertion point;
                break 'outer i;
            }

            // The entire (rest of the) registry contains items matching
            // `matcher`, but none matching the given `type_id` -> insert at
            // the end.
            registry.len()
        };

        registry.insert(
            insert_at,
            BuilderRegistryEntry {
                matcher,
                ty: type_id,
                builder,
            },
        );
    }

    /// Reserve space for at least `n` additional types.
    pub fn reserve(&self, n: usize) {
        self.inner.lock().unwrap().reserve(n);
    }

    /// Add a new builder to the registry.
    ///
    /// This allows to add any `FromXml` implementation whose output can be
    /// converted to `T`.
    pub fn add<U: Any + FromXml>(&self)
    where
        T: MayContain<U>,
    {
        struct Wrapper<B, X: ?Sized> {
            inner: B,
            output: PhantomData<X>,
        }

        impl<X: ?Sized, O, B: FromEventsBuilder<Output = O>> FromEventsBuilder for Wrapper<B, X>
        where
            X: MayContain<O>,
        {
            type Output = Box<X>;

            fn feed(
                &mut self,
                ev: rxml::Event,
                ctx: &Context<'_>,
            ) -> Result<Option<Self::Output>, Error> {
                self.inner
                    .feed(ev, ctx)
                    .map(|x| x.map(|x| <X as MayContain<O>>::upcast_into(x)))
            }
        }

        self.insert(
            U::xml_name_matcher(),
            TypeId::of::<U>(),
            Box::new(|name, attrs, ctx| {
                U::from_events(name, attrs, ctx).map(|builder| {
                    Box::new(Wrapper {
                        inner: builder,
                        output: PhantomData,
                    }) as Box<dyn FromEventsBuilder<Output = Box<T>>>
                })
            }),
        )
    }

    fn try_build(
        inner: &mut Vec<BuilderRegistryEntry<T>>,
        mut name: rxml::QName,
        mut attrs: rxml::AttrMap,
        ctx: &Context<'_>,
        matcher_builder: impl for<'x> FnOnce(&'x rxml::QName) -> XmlNameMatcher<'x>,
    ) -> Result<Box<dyn FromEventsBuilder<Output = Box<T>>>, FromEventsError> {
        let matcher = matcher_builder(&name);
        let start_scan_at = inner.partition_point(|entry| entry.matcher < matcher);

        for entry in &inner[start_scan_at..] {
            if !entry.matcher.matches(&name) {
                return Err(FromEventsError::Mismatch { name, attrs });
            }

            match (entry.builder)(name, attrs, ctx) {
                Ok(v) => return Ok(v),
                Err(FromEventsError::Invalid(e)) => return Err(FromEventsError::Invalid(e)),
                Err(FromEventsError::Mismatch {
                    name: new_name,
                    attrs: new_attrs,
                }) => {
                    name = new_name;
                    attrs = new_attrs;
                }
            }
        }

        Err(FromEventsError::Mismatch { name, attrs })
    }

    /// Make a builder for the given element header.
    ///
    /// This tries all applicable `FromXml` implementations which have
    /// previously been added via [`add`][`Self::add`] in unspecified order.
    /// The first implementation to either fail or succeed at constructing a
    /// builder determines the result. Implementations which return a
    /// [`FromEventsError::Mismatch`][`crate::error::FromEventsError::Mismatch`]
    /// are ignored.
    ///
    /// If all applicable implementations return `Mismatch`, this function
    /// returns `Mismatch`, too.
    pub fn make_builder(
        &self,
        name: rxml::QName,
        attrs: rxml::AttrMap,
        ctx: &Context<'_>,
    ) -> Result<Box<dyn FromEventsBuilder<Output = Box<T>>>, FromEventsError> {
        let mut inner = self.inner.lock().unwrap();
        let (name, attrs) = match Self::try_build(&mut *inner, name, attrs, ctx, |qname| {
            XmlNameMatcher::Specific(qname.0.as_str(), qname.1.as_str())
        }) {
            Ok(v) => return Ok(v),
            Err(FromEventsError::Invalid(e)) => return Err(FromEventsError::Invalid(e)),
            Err(FromEventsError::Mismatch { name, attrs }) => (name, attrs),
        };

        let (name, attrs) = match Self::try_build(&mut *inner, name, attrs, ctx, |qname| {
            XmlNameMatcher::InNamespace(qname.0.as_str())
        }) {
            Ok(v) => return Ok(v),
            Err(FromEventsError::Invalid(e)) => return Err(FromEventsError::Invalid(e)),
            Err(FromEventsError::Mismatch { name, attrs }) => (name, attrs),
        };

        Self::try_build(&mut *inner, name, attrs, ctx, |_| XmlNameMatcher::Any)
    }
}

/// # Dynamic XSO type
///
/// This trait provides the infrastructure for dynamic XSO types. In
/// particular, it provides:
///
/// - Access to a [`BuilderRegistry`] which allows constructing an instance of
///   the dynamic XSO type from XML.
/// - Downcasts to specific types.
///
/// Like [`MayContain`], it is typically implemented on `dyn Trait` for some
/// `Trait` and it is best generated using
/// [`derive_dyn_traits`][`crate::derive_dyn_traits`].
///
/// This trait explicitly provides the methods provided by [`Any`]. The reason
/// for this duplication is that with `DynXso` being intended to be
/// implemented on `dyn Trait`, code using this trait cannot cast the value
/// to `dyn Any` to access the `downcast`-related methods (`type_id` would,
/// in fact, work if `DynXso` had a bound on `Any`, but not the downcasts).
///
/// *Hint*: It should not be necessary for user code to directly interact
/// with this trait.
pub trait DynXso: 'static {
    /// Return the builder registry for this dynamic type.
    ///
    /// The builder registry is used by [`Xso::register_type`] and the
    /// `FromXml` implementation.
    fn registry() -> &'static BuilderRegistry<Self>;

    /// Try to downcast a boxed dynamic XSO to a specific type.
    ///
    /// If `self` contains a `T` (and thus, the downcast succeeds), `Ok(_)`
    /// is returned. Otherwise, `Err(self)` is returned, allowing to chain
    /// this function with other downcast attempts.
    ///
    /// This is similar to `downcast` on [`dyn Any`][`core::any::Any`].
    fn try_downcast<T: 'static>(self: Box<Self>) -> Result<Box<T>, Box<Self>>
    where
        Self: MayContain<T>;

    /// Try to downcast a dynamic XSO to a reference to a specific type.
    ///
    /// If `self` contains a `T` (and thus, the downcast succeeds), `Some(_)`
    /// is returned. Otherwise, `None`.
    ///
    /// This is similar to `downcast_ref` on [`dyn Any`][`core::any::Any`].
    fn try_downcast_ref<T: 'static>(&self) -> Option<&T>
    where
        Self: MayContain<T>;

    /// Try to downcast a dynamic XSO to a mutable reference to a specific
    /// type.
    ///
    /// If `self` contains a `T` (and thus, the downcast succeeds), `Some(_)`
    /// is returned. Otherwise, `None`.
    ///
    /// This is similar to `downcast_mut` on [`dyn Any`][`core::any::Any`].
    fn try_downcast_mut<T: 'static>(&mut self) -> Option<&mut T>
    where
        Self: MayContain<T>;

    /// Return true if `self` contains a `T`.
    ///
    /// This is similar to `is` on [`dyn Any`][`core::any::Any`].
    fn is<T: 'static>(&self) -> bool
    where
        Self: MayContain<T>;

    /// Return the [`TypeId`] of `self`.
    ///
    /// This is similar to `type_id` on [`dyn Any`][`core::any::Any`].
    fn type_id(&self) -> TypeId;
}

/// # Declare that `T` may be held by `Box<Self>`
///
/// This trait is used to constrain which types can be put in
/// [`Xso<Self>`][`Xso`]. It is typically implemented on `dyn Trait` for all
/// `T: Trait`.
///
/// To automatically generate suitable implementations of this trait, see
/// [`derive_dyn_traits`][`crate::derive_dyn_traits`].
///
/// Implementation-wise, this trait is very similar to `Box<Self>: From<T>`.
/// However, `From` is also used in many different circumstances and it cannot
/// be suitably overloaded on `Box<_>`, so a new trait was introduced for this
/// particular purpose.
pub trait MayContain<T> {
    /// Convert a value of `T` into `Box<Self>`.
    fn upcast_into(other: T) -> Box<Self>;
}

/// # Dynamic XSO container
///
/// This container is very similar to `Box<_>`, but geared specifically toward
/// the use with `T` being a `dyn Trait`. It also implements [`FromXml`]
/// (unconditionally) and [`AsXml`] if `T` implements [`AsXmlDyn`].
///
/// In order to provide these features, `T` must implement [`DynXso`] and
/// [`MayContain`]. Implementations for these traits can be generated using
/// [`derive_dyn_traits`][`crate::derive_dyn_traits`].
///
/// Most methods on `Xso<dyn Trait>` which take type parameters are only
/// available for types `U` implementing `Trait` (or, more precisely, where
/// `dyn Trait` implements `MayContain<U>`).
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[repr(transparent)]
pub struct Xso<T: ?Sized> {
    inner: Box<T>,
}

impl<T: ?Sized> Deref for Xso<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T: ?Sized> DerefMut for Xso<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<T: DynXso + ?Sized> fmt::Debug for Xso<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Xso")
            .field("inner", &self.inner_type_id())
            .finish()
    }
}

impl<T: ?Sized> Xso<T> {
    /// Wrap a value into a `Xso<dyn Trait>`.
    ///
    /// ```
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits};
    /// trait Trait: Any {}
    /// derive_dyn_traits!(Trait);
    ///
    /// struct Foo;
    /// impl Trait for Foo {}
    ///
    /// let x: Xso<dyn Trait> = Xso::wrap(Foo);
    /// ```
    pub fn wrap<U: 'static>(value: U) -> Self
    where
        T: MayContain<U>,
    {
        Self {
            inner: T::upcast_into(value),
        }
    }

    /// Convert `Xso<T>` into `Box<T>`.
    ///
    /// ```
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits};
    /// trait Trait: Any {}
    /// derive_dyn_traits!(Trait);
    ///
    /// struct Foo;
    /// impl Trait for Foo {}
    ///
    /// let x: Xso<dyn Trait> = Xso::wrap(Foo);
    /// let x: Box<dyn Trait> = x.into_boxed();
    /// ```
    pub fn into_boxed(self) -> Box<T> {
        self.inner
    }
}

impl<T: DynXso + ?Sized + 'static> Xso<T> {
    /// Downcast `self` to `Box<U>`.
    ///
    /// If the downcast fails, `self` is returned without change.
    ///
    /// ```
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits};
    /// trait Trait: Any {}
    /// derive_dyn_traits!(Trait);
    ///
    /// struct Foo;
    /// impl Trait for Foo {}
    ///
    /// struct Bar;
    /// impl Trait for Bar {}
    ///
    /// let x: Xso<dyn Trait> = Xso::wrap(Foo);
    /// // Does not contain a Bar, so downcast fails.
    /// let x: Xso<dyn Trait> = x.downcast::<Bar>().err().unwrap();
    /// // *Does* contain a Foo, so downcast succeeds.
    /// let f: Foo = *x.downcast().unwrap();
    /// ```
    pub fn downcast<U: 'static>(self) -> Result<Box<U>, Self>
    where
        T: MayContain<U>,
    {
        match self.inner.try_downcast() {
            Ok(v) => Ok(v),
            Err(inner) => Err(Self { inner }),
        }
    }

    /// Downcast `&self` to `&U`.
    ///
    /// ```
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits};
    /// trait Trait: Any {}
    /// derive_dyn_traits!(Trait);
    ///
    /// struct Foo;
    /// impl Trait for Foo {}
    ///
    /// struct Bar;
    /// impl Trait for Bar {}
    ///
    /// let x: Xso<dyn Trait> = Xso::wrap(Foo);
    /// // Does not contain a Bar, so downcast fails.
    /// assert!(x.downcast_ref::<Bar>().is_none());
    /// // *Does* contain a Foo, so downcast succeeds.
    /// let f: &Foo = x.downcast_ref().unwrap();
    /// ```
    pub fn downcast_ref<U: 'static>(&self) -> Option<&U>
    where
        T: MayContain<U>,
    {
        self.inner.try_downcast_ref()
    }

    /// Downcast `&mut self` to `&mut U`.
    ///
    /// ```
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits};
    /// trait Trait: Any {}
    /// derive_dyn_traits!(Trait);
    ///
    /// struct Foo;
    /// impl Trait for Foo {}
    ///
    /// struct Bar;
    /// impl Trait for Bar {}
    ///
    /// let mut x: Xso<dyn Trait> = Xso::wrap(Foo);
    /// // Does not contain a Bar, so downcast fails.
    /// assert!(x.downcast_mut::<Bar>().is_none());
    /// // *Does* contain a Foo, so downcast succeeds.
    /// let f: &mut Foo = x.downcast_mut().unwrap();
    /// ```
    pub fn downcast_mut<U: 'static>(&mut self) -> Option<&mut U>
    where
        T: MayContain<U>,
    {
        self.inner.try_downcast_mut()
    }

    fn inner_type_id(&self) -> TypeId {
        (&self.inner as &dyn Any).type_id()
    }

    /// Register a new type to be constructible.
    ///
    /// Only types registered through this function can be parsed from XML via
    /// the [`FromXml`] implementation on `Xso<T>`. See
    /// [`dynxso`][`crate::dynxso`] for details.
    ///
    #[cfg_attr(
        not(feature = "macros"),
        doc = "Because the macros feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
    )]
    #[cfg_attr(feature = "macros", doc = "\n```\n")]
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits, from_bytes, FromXml};
    /// trait Trait: Any {}
    /// derive_dyn_traits!(Trait);
    ///
    /// #[derive(FromXml, PartialEq, Debug)]
    /// #[xml(namespace = "urn:example", name = "foo")]
    /// struct Foo;
    /// impl Trait for Foo {}
    ///
    /// // Parsing fails, because register_type() has not been called for
    /// // Foo:
    /// assert!(from_bytes::<Xso<dyn Trait>>("<foo xmlns='urn:example'/>".as_bytes()).is_err());
    ///
    /// Xso::<dyn Trait>::register_type::<Foo>();
    /// // After registering Foo with Xso<dyn Trait>, parsing succeeds and
    /// // we can downcast to Foo:
    /// let x: Xso<dyn Trait> = from_bytes("<foo xmlns='urn:example'/>".as_bytes()).unwrap();
    /// assert_eq!(Foo, *x.downcast().unwrap());
    /// ```
    pub fn register_type<U: FromXml + 'static>()
    where
        T: MayContain<U>,
    {
        T::registry().add::<U>()
    }
}

/// Wrapper around a `FromEventsBuilder` to convert a `Box<T>` output to a
/// `Xso<T>` output.
///
/// Not constructible by users, only for internal use.
pub struct DynBuilder<B> {
    inner: B,
}

impl<T: DynXso + ?Sized + 'static, B: FromEventsBuilder<Output = Box<T>>> FromEventsBuilder
    for DynBuilder<B>
{
    type Output = Xso<T>;

    fn feed(&mut self, ev: rxml::Event, ctx: &Context) -> Result<Option<Self::Output>, Error> {
        self.inner
            .feed(ev, ctx)
            .map(|x| x.map(|inner| Xso { inner }))
    }
}

/// Wrapper around a `FromEventsBuilder` to convert a `Box<T>` output to a
/// `T` output.
pub struct UnboxBuilder<T> {
    inner: T,
}

impl<O, T: FromEventsBuilder<Output = Box<O>>> UnboxBuilder<T> {
    /// Wrap a `FromEventsBuilder` which generates `Box<O>`.
    pub fn wrap(inner: T) -> Self {
        Self { inner }
    }
}

impl<O, T: FromEventsBuilder<Output = Box<O>>> FromEventsBuilder for UnboxBuilder<T> {
    type Output = O;

    fn feed(&mut self, ev: rxml::Event, ctx: &Context) -> Result<Option<Self::Output>, Error> {
        self.inner.feed(ev, ctx).map(|x| x.map(|inner| *inner))
    }
}

impl<T: DynXso + ?Sized + 'static> FromXml for Xso<T> {
    type Builder = DynBuilder<Box<dyn FromEventsBuilder<Output = Box<T>>>>;

    fn from_events(
        name: rxml::QName,
        attrs: rxml::AttrMap,
        ctx: &Context<'_>,
    ) -> Result<Self::Builder, FromEventsError> {
        T::registry()
            .make_builder(name, attrs, ctx)
            .map(|inner| DynBuilder { inner })
    }
}

impl<T: DynXso + AsXmlDyn + ?Sized + 'static> AsXml for Xso<T> {
    type ItemIter<'x> = Box<dyn Iterator<Item = Result<Item<'x>, Error>> + 'x>;

    fn as_xml_iter(&self) -> Result<Self::ItemIter<'_>, Error> {
        self.inner.as_xml_dyn_iter()
    }

    fn as_xml_dyn_iter(&self) -> Result<Self::ItemIter<'_>, Error> {
        self.inner.as_xml_dyn_iter()
    }
}
