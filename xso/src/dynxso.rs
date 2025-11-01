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
    not(all(feature = "macros", feature = "std")),
    doc = "Because the macros feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
)]
#![cfg_attr(all(feature = "macros", feature = "std"), doc = "\n```\n")]
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

use alloc::{
    boxed::Box,
    collections::{
        btree_map::{self, Entry},
        BTreeMap,
    },
    vec::{self, Vec},
};
#[cfg(feature = "std")]
use core::marker::PhantomData;
use core::{
    any::{Any, TypeId},
    fmt,
    ops::{Deref, DerefMut},
    slice,
};
#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(feature = "std")]
use crate::fromxml::XmlNameMatcher;
use crate::{
    asxml::AsXmlDyn,
    error::{Error, FromEventsError},
    AsXml, Context, FromEventsBuilder, FromXml, Item,
};

/// # Generate `DynXso` and `MayContain` trait implementations
///
/// This macro generates trait [`DynXso`] and [`MayContain`] trait
/// implementations for a given trait. For more background information on when
/// that is a useful thing to have, see the [`dynxso`][`crate::dynxso`]
/// module.
///
/// ## Syntax
///
/// This macro can be called in two forms:
///
/// - `derive_dyn_traits!(Trait)` uses the default [`BuilderRegistry`]
///    as [`DynXso::Registry`] type and is only available if `xso` is built
///    with the `"std"` feature.
/// - `derive_dyn_traits!(Trait use Type = expr)` where `Type` is used as
///   [`DynXso::Registry`], initialized with `expr`. This form is available
///   for any set of crate features.
///
/// ## Example
///
/// When `std` is enabled, the simple syntax can be used.
#[cfg_attr(
    not(feature = "std"),
    doc = "Because the std feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
)]
#[cfg_attr(feature = "std", doc = "\n```\n")]
/// # use core::any::Any;
/// use xso::derive_dyn_traits;
/// trait Foo: Any {}
/// derive_dyn_traits!(Foo);
/// ```
///
/// Note that the trait this macro is called on **must** have a bound on
/// `Any`, otherwise the generated code will not compile:
///
#[cfg_attr(
    not(feature = "std"),
    doc = "Because the std feature was not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
)]
#[cfg_attr(feature = "std", doc = "\n```compile_fail\n")]
/// use xso::derive_dyn_traits;
/// trait Foo {}
/// derive_dyn_traits!(Foo);
/// // ↑ will generate a bunch of errors about incompatible types
/// ```
///
/// If the `std` feature is not enabled or if you want to use another
/// `Registry` for whichever reason, the explicit form can be used:
///
/// ```
/// # use core::any::Any;
/// use xso::derive_dyn_traits;
/// trait Foo: Any {}
/// struct Registry { /* .. */ }
/// derive_dyn_traits!(Foo use Registry = Registry { /* .. */ });
/// ```
///
/// In that case, you should review the trait requirements of the
/// [`DynXso::Registry`] associated type.
#[macro_export]
macro_rules! derive_dyn_traits {
    ($trait:ident use $registry:ty = $reginit:expr) => {
        impl $crate::dynxso::DynXso for dyn $trait {
            type Registry = $registry;

            fn registry() -> &'static Self::Registry {
                static DATA: $registry = $reginit;
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
    ($trait:ident) => {
        $crate::_internal_derive_dyn_traits_std_only!($trait);
    };
}

#[macro_export]
#[doc(hidden)]
#[cfg(feature = "std")]
macro_rules! _internal_derive_dyn_traits_std_only {
    ($trait:ident) => {
        $crate::derive_dyn_traits!($trait use $crate::dynxso::BuilderRegistry<dyn $trait> = $crate::dynxso::BuilderRegistry::new());
    };
}

#[macro_export]
#[doc(hidden)]
#[cfg(not(feature = "std"))]
macro_rules! _internal_derive_dyn_traits_std_only {
    ($trait:ident) => {
        compile_error!(concat!("derive_dyn_traits!(", stringify!($trait), ") can only be used if the xso crate has been built with the \"std\" feature enabled. Without \"std\", the explicit form of derive_dyn_traits!(", stringify!($trait), " use .. = ..) must be used (see docs)."));
    };
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
struct BuilderRegistryEntry<T: ?Sized> {
    matcher: XmlNameMatcher<'static>,
    ty: TypeId,
    builder: BuilderRegistryBuilder<T>,
}

#[cfg(feature = "std")]
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
/// For general information on builder registries, see [`registry`].
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
/// # use xso::{dynxso::{BuilderRegistry, MayContain, registry::{DynXsoRegistryAdd, DynXsoRegistryLookup}}, FromXml, from_bytes, exports::rxml::{Namespace, AttrMap, QName, Event, parser::EventMetrics, xml_ncname}, Context, FromEventsBuilder, error::FromEventsError};
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
/// # use xso::{dynxso::{BuilderRegistry, MayContain, registry::{DynXsoRegistryAdd, DynXsoRegistryLookup}}, FromXml, from_bytes, exports::rxml::{Namespace, AttrMap, QName, Event, parser::EventMetrics}, Context, FromEventsBuilder, error::FromEventsError};
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
#[cfg(feature = "std")]
pub struct BuilderRegistry<T: ?Sized> {
    inner: Mutex<Vec<BuilderRegistryEntry<T>>>,
}

#[cfg(feature = "std")]
impl<T: ?Sized + 'static> Default for BuilderRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
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

    fn try_build(
        inner: &mut [BuilderRegistryEntry<T>],
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
}

#[cfg(feature = "std")]
impl<T: ?Sized + 'static> DynXsoRegistryAdd<T> for BuilderRegistry<T> {
    fn add<U: Any + FromXml>(&self)
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
}

#[cfg(feature = "std")]
impl<T: ?Sized + 'static> DynXsoRegistryLookup<T> for BuilderRegistry<T> {
    fn make_builder(
        &self,
        name: rxml::QName,
        attrs: rxml::AttrMap,
        ctx: &Context<'_>,
    ) -> Result<Box<dyn FromEventsBuilder<Output = Box<T>>>, FromEventsError> {
        let mut inner = self.inner.lock().unwrap();
        let (name, attrs) = match Self::try_build(&mut inner, name, attrs, ctx, |qname| {
            XmlNameMatcher::Specific(qname.0.as_str(), qname.1.as_str())
        }) {
            Ok(v) => return Ok(v),
            Err(FromEventsError::Invalid(e)) => return Err(FromEventsError::Invalid(e)),
            Err(FromEventsError::Mismatch { name, attrs }) => (name, attrs),
        };

        let (name, attrs) = match Self::try_build(&mut inner, name, attrs, ctx, |qname| {
            XmlNameMatcher::InNamespace(qname.0.as_str())
        }) {
            Ok(v) => return Ok(v),
            Err(FromEventsError::Invalid(e)) => return Err(FromEventsError::Invalid(e)),
            Err(FromEventsError::Mismatch { name, attrs }) => (name, attrs),
        };

        Self::try_build(&mut inner, name, attrs, ctx, |_| XmlNameMatcher::Any)
    }
}

/// # Helper traits for dynamic XSO builder registries.
///
/// Builder registries hold type-erased [`FromXml::from_events`]
/// implementations. Registries can be used to dynamically dispatch to a set
/// of `FromXml` implementations which is not known at compile time.
///
/// Under the hood, they are used by the `FromXml` implementation on
/// [`Xso<T>`][`Xso`], via the [`DynXso::Registry`] type.
///
/// Note that registries generally do not allow to add arbitrary builders. All
/// builders must originate in a [`FromXml`] implementation and their output
/// must be convertible to the specific type the registry is defined for.
///
/// The default implementation is [`BuilderRegistry`], which is only available
/// if `xso` is built with the `"std"` feature due to the inherent need for a
/// `Mutex`.
pub mod registry {
    use super::*;

    /// Trait for a builder registry supports constructing elements.
    pub trait DynXsoRegistryLookup<T: ?Sized> {
        /// Make a builder for the given element header.
        ///
        /// This tries all applicable `FromXml` implementations which have
        /// previously been added via [`add`][`DynXsoRegistryAdd::add`] in
        /// unspecified order. The first implementation to either fail or
        /// succeed at constructing a builder determines the result.
        /// Implementations which return a
        /// [`FromEventsError::Mismatch`][`crate::error::FromEventsError::Mismatch`]
        /// are ignored.
        ///
        /// If all applicable implementations return `Mismatch`, this function
        /// returns `Mismatch`, too.
        fn make_builder(
            &self,
            name: rxml::QName,
            attrs: rxml::AttrMap,
            ctx: &Context<'_>,
        ) -> Result<Box<dyn FromEventsBuilder<Output = Box<T>>>, FromEventsError>;
    }

    /// Trait for a builder registry supports registering new builders at
    /// runtime.
    pub trait DynXsoRegistryAdd<T: ?Sized> {
        /// Add a new builder to the registry.
        ///
        /// This allows to add any `FromXml` implementation whose output can be
        /// converted to `T`.
        fn add<U: Any + FromXml>(&self)
        where
            T: MayContain<U>;
    }
}

use registry::*;

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
    /// Builder registry type for this dynamic type.
    ///
    /// The `Registry` type *should* implement the following traits:
    ///
    /// - [`DynXsoRegistryAdd`] is required to make
    ///   [`Xso::<Self>::register_type()`][`Xso::register_type`] available.
    /// - [`DynXsoRegistryLookup`] is required to make [`FromXml`] available
    ///   on [`Xso<Self>`][`Xso`] (and, by extension, on
    ///   [`XsoVec<Self>`][`XsoVec`]).
    ///
    /// However, any type with static lifetime can be used, even without the
    /// trait implementations above, if the limitations are acceptable.
    type Registry: 'static;

    /// Return the builder registry for this dynamic type.
    ///
    /// See [`Registry`][`Self::Registry`] for details.
    fn registry() -> &'static Self::Registry;

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
    #[cfg_attr(feature = "std", doc = "derive_dyn_traits!(Trait);")]
    #[cfg_attr(not(feature = "std"), doc = "derive_dyn_traits!(Trait use () = ());")]
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
    #[cfg_attr(feature = "std", doc = "derive_dyn_traits!(Trait);")]
    #[cfg_attr(not(feature = "std"), doc = "derive_dyn_traits!(Trait use () = ());")]
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
    #[cfg_attr(feature = "std", doc = "derive_dyn_traits!(Trait);")]
    #[cfg_attr(not(feature = "std"), doc = "derive_dyn_traits!(Trait use () = ());")]
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

    fn force_downcast<U: 'static>(self) -> Box<U>
    where
        T: MayContain<U>,
    {
        match self.downcast::<U>() {
            Ok(v) => v,
            Err(v) => panic!(
                "force_downcast called on mismatching types: requested {:?} ({}) != actual {:?}",
                TypeId::of::<U>(),
                core::any::type_name::<U>(),
                v.inner_type_id()
            ),
        }
    }

    /// Downcast `&self` to `&U`.
    ///
    /// ```
    /// # use core::any::Any;
    /// # use xso::{dynxso::Xso, derive_dyn_traits};
    /// trait Trait: Any {}
    #[cfg_attr(feature = "std", doc = "derive_dyn_traits!(Trait);")]
    #[cfg_attr(not(feature = "std"), doc = "derive_dyn_traits!(Trait use () = ());")]
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
    #[cfg_attr(feature = "std", doc = "derive_dyn_traits!(Trait);")]
    #[cfg_attr(not(feature = "std"), doc = "derive_dyn_traits!(Trait use () = ());")]
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
        DynXso::type_id(&*self.inner)
    }
}

impl<R: DynXsoRegistryAdd<T> + 'static, T: DynXso<Registry = R> + ?Sized + 'static> Xso<T> {
    /// Register a new type to be constructible.
    ///
    /// Only types registered through this function can be parsed from XML via
    /// the [`FromXml`] implementation on `Xso<T>`. See
    /// [`dynxso`][`crate::dynxso`] for details.
    ///
    #[cfg_attr(
        not(all(feature = "macros", feature = "std")),
        doc = "Because the macros and std features were not enabled at doc build time, the example cannot be tested.\n\n```ignore\n"
    )]
    #[cfg_attr(all(feature = "macros", feature = "std"), doc = "\n```\n")]
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

impl<R: DynXsoRegistryLookup<T> + 'static, T: DynXso<Registry = R> + ?Sized + 'static> FromXml
    for Xso<T>
{
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

/// Error type for retrieving a single item from `XsoVec`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TakeOneError {
    /// More than one item was found.
    MultipleEntries,
}

impl fmt::Display for TakeOneError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MultipleEntries => f.write_str("multiple entries found"),
        }
    }
}

/// # Container for dynamically-typed XSOs optimized for type-keyed access
///
/// This container holds dynamically typed XSOs (see
/// [`Xso<dyn Trait>`][`Xso`]). It allows efficient access to its contents
/// based on the actual type.
///
/// Like `Xso<dyn Trait>` itself, `XsoVec<dyn Trait>` requires that
/// `MayContain` is implemented by `dyn Trait` for all items which are added
/// to the container. This is automatically the case for all `T: Trait`
/// if [`derive_dyn_traits`][`crate::derive_dyn_traits`] has been used on
/// `Trait`.
///
/// Note that `XsoVec` has a non-obvious iteration order, which is described
/// in [`XsoVec::iter()`][`Self::iter`].
pub struct XsoVec<T: ?Sized> {
    inner: BTreeMap<TypeId, Vec<Xso<T>>>,
}

impl<T: ?Sized> fmt::Debug for XsoVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "XsoVec[{} types, {} items]",
            self.inner.len(),
            self.len()
        )
    }
}

impl<T: ?Sized> Default for XsoVec<T> {
    fn default() -> Self {
        Self {
            inner: BTreeMap::default(),
        }
    }
}

impl<T: DynXso + ?Sized + 'static> XsoVec<T> {
    /// Construct a new, empty `XsoVec`.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Return a reference to the first item of type `U`.
    ///
    /// If the container does not hold any item of type `U`, return `None`.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    /// assert_eq!(vec.get_first::<Foo>(), Some(&Foo(2)));
    /// assert_eq!(vec.get_first::<Bar>(), Some(&Bar(1)));
    /// assert_eq!(vec.get_first::<Baz>(), None);
    ///
    /// ```
    pub fn get_first<U: 'static>(&self) -> Option<&U>
    where
        T: MayContain<U>,
    {
        self.iter_typed::<U>().next()
    }

    /// Return a mutable reference to the first item of type `U`.
    ///
    /// If the container does not hold any item of type `U`, return `None`.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Foo(1));
    /// vec.get_first_mut::<Foo>().unwrap().0 = 2;
    /// assert_eq!(vec.get_first::<Foo>(), Some(&Foo(2)));
    /// ```
    pub fn get_first_mut<U: 'static>(&mut self) -> Option<&mut U>
    where
        T: MayContain<U>,
    {
        self.iter_typed_mut::<U>().next()
    }

    /// Take and return exactly one item of type `U`.
    ///
    /// If no item of type `U` is present in the container, return Ok(None).
    /// If more than one item of type `U` is present in the container,
    /// return an error.
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    /// assert_eq!(vec.take_one::<Foo>(), Err(TakeOneError::MultipleEntries));
    /// assert_eq!(*vec.take_one::<Bar>().unwrap().unwrap(), Bar(1));
    /// assert_eq!(vec.take_one::<Bar>(), Ok(None));
    /// assert_eq!(vec.take_one::<Baz>(), Ok(None));
    /// ```
    pub fn take_one<U: 'static>(&mut self) -> Result<Option<Box<U>>, TakeOneError>
    where
        T: MayContain<U>,
    {
        let source = match self.inner.get_mut(&TypeId::of::<U>()) {
            Some(v) => v,
            None => return Ok(None),
        };
        if source.len() > 1 {
            return Err(TakeOneError::MultipleEntries);
        }
        Ok(source.pop().map(Xso::force_downcast))
    }

    /// Take and return the first item of type `U`.
    ///
    /// If no item of type `U` is present in the container, return None.
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    /// assert_eq!(*vec.take_first::<Foo>().unwrap(), Foo(2));
    /// assert_eq!(*vec.take_first::<Foo>().unwrap(), Foo(1));
    /// assert_eq!(*vec.take_first::<Bar>().unwrap(), Bar(1));
    /// assert_eq!(vec.take_first::<Bar>(), None);
    /// assert_eq!(vec.take_first::<Baz>(), None);
    /// ```
    pub fn take_first<U: 'static>(&mut self) -> Option<Box<U>>
    where
        T: MayContain<U>,
    {
        let source = self.inner.get_mut(&TypeId::of::<U>())?;
        if source.len() == 0 {
            return None;
        }
        Some(source.remove(0).force_downcast())
    }

    /// Take and return the last item of type `U`.
    ///
    /// If no item of type `U` is present in the container, return None.
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    /// assert_eq!(*vec.take_last::<Foo>().unwrap(), Foo(1));
    /// assert_eq!(*vec.take_last::<Foo>().unwrap(), Foo(2));
    /// assert_eq!(*vec.take_last::<Bar>().unwrap(), Bar(1));
    /// assert_eq!(vec.take_last::<Bar>(), None);
    /// assert_eq!(vec.take_last::<Baz>(), None);
    /// ```
    pub fn take_last<U: 'static>(&mut self) -> Option<Box<U>>
    where
        T: MayContain<U>,
    {
        let source = self.inner.get_mut(&TypeId::of::<U>())?;
        source.pop().map(Xso::force_downcast)
    }

    /// Iterate all items of type `U` as references.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    ///
    /// let foos: Vec<_> = vec.iter_typed::<Foo>().collect();
    /// assert_eq!(&foos[..], &[&Foo(2), &Foo(1)]);
    /// ```
    pub fn iter_typed<U: 'static>(&self) -> impl Iterator<Item = &U>
    where
        T: MayContain<U>,
    {
        let iter = match self.inner.get(&TypeId::of::<U>()) {
            Some(v) => v.deref().iter(),
            None => [].iter(),
        };
        // UNWRAP: We group the values by TypeId, so the downcast should never
        // fail, but I am too chicken to use the unchecked variants :).
        iter.map(|x| x.downcast_ref::<U>().unwrap())
    }

    /// Iterate all items of type `U` as mutable references.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    ///
    /// let foos: Vec<_> = vec.iter_typed_mut::<Foo>().collect();
    /// assert_eq!(&foos[..], &[&mut Foo(2), &mut Foo(1)]);
    /// ```
    pub fn iter_typed_mut<U: 'static>(&mut self) -> impl Iterator<Item = &mut U>
    where
        T: MayContain<U>,
    {
        let iter = match self.inner.get_mut(&TypeId::of::<U>()) {
            Some(v) => v.deref_mut().iter_mut(),
            None => [].iter_mut(),
        };
        // UNWRAP: We group the values by TypeId, so the downcast should never
        // fail, but I am too chicken to use the unchecked variants :).
        iter.map(|x| x.downcast_mut::<U>().unwrap())
    }

    /// Drain all items of type `U` out of the container.
    ///
    /// If the result is dropped before the end of the iterator has been
    /// reached, the remaining items are still dropped out of the container.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Baz(u32);
    /// impl Trait for Baz {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    /// vec.push(Foo(1));
    ///
    /// let foos: Vec<_> = vec.drain_typed::<Foo>().map(|x| *x).collect();
    /// //                             converts Box<T> to T ↑
    /// assert_eq!(&foos[..], &[Foo(2), Foo(1)]);
    /// ```
    pub fn drain_typed<U: 'static>(&mut self) -> impl Iterator<Item = Box<U>>
    where
        T: MayContain<U>,
    {
        let iter = match self.inner.remove(&TypeId::of::<U>()) {
            Some(v) => v.into_iter(),
            None => Vec::new().into_iter(),
        };
        // UNWRAP: We group the values by TypeId, so the downcast should never
        // fail, but I am too chicken to use the unchecked variants :).
        iter.map(|x| match x.downcast::<U>() {
            Ok(v) => v,
            Err(_) => {
                unreachable!("TypeId disagrees with Xso<_>::downcast, or internal state corruption")
            }
        })
    }

    fn ensure_vec_mut_for(&mut self, type_id: TypeId) -> &mut Vec<Xso<T>> {
        match self.inner.entry(type_id) {
            Entry::Vacant(v) => v.insert(Vec::new()),
            Entry::Occupied(o) => o.into_mut(),
        }
    }

    /// Push a new item of type `U` to the end of the section of `U` inside
    /// the container.
    ///
    /// Please note the information about iteration order of the `XsoVec`
    /// at [`XsoVec::iter`][`Self::iter`].
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Foo(1));
    /// ```
    pub fn push<U: 'static>(&mut self, value: U)
    where
        T: MayContain<U>,
    {
        self.ensure_vec_mut_for(TypeId::of::<U>())
            .push(Xso::wrap(value));
    }

    /// Push a new dynamically typed item to the end of the section of values
    /// with the same type inside the container.
    ///
    /// Please note the information about iteration order of the `XsoVec`
    /// at [`XsoVec::iter`][`Self::iter`].
    ///
    /// ```
    /// # use xso::dynxso::Xso;
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u8);
    /// impl Trait for Bar {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Foo(1));
    /// vec.push_dyn(Xso::wrap(Foo(2)));
    /// vec.push_dyn(Xso::wrap(Bar(1)));
    /// vec.push(Bar(2));
    ///
    /// let foos: Vec<_> = vec.iter_typed::<Foo>().collect();
    /// assert_eq!(&foos[..], &[&Foo(1), &Foo(2)]);
    ///
    /// let bars: Vec<_> = vec.iter_typed::<Bar>().collect();
    /// assert_eq!(&bars[..], &[&Bar(1), &Bar(2)]);
    /// ```
    pub fn push_dyn(&mut self, value: Xso<T>) {
        self.ensure_vec_mut_for(value.inner_type_id()).push(value);
    }
}

impl<T: ?Sized> XsoVec<T> {
    /// Clear all contents, without deallocating memory.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Foo(1));
    /// vec.push(Foo(2));
    /// vec.clear();
    /// assert_eq!(vec.len(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.inner.values_mut().for_each(|x| x.clear());
    }

    /// Return true if there are no items in the container.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// assert!(vec.is_empty());
    /// vec.push(Foo(1));
    /// assert!(!vec.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.values().all(|x| x.is_empty())
    }

    /// Reduce memory use of the container to the minimum required to hold
    /// the current data.
    ///
    /// This may be expensive if lots of data needs to be shuffled.
    pub fn shrink_to_fit(&mut self) {
        self.inner.retain(|_, x| {
            if x.is_empty() {
                return false;
            }
            x.shrink_to_fit();
            true
        });
    }

    /// Return the total amount of items in the container.
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// assert_eq!(vec.len(), 0);
    /// vec.push(Foo(1));
    /// assert_eq!(vec.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.inner.values().map(|x| x.len()).sum()
    }

    /// Iterate the items inside the container.
    ///
    /// This iterator (unlike the iterator returned by
    /// [`iter_typed()`][`Self::iter_typed`]) yields references to **untyped**
    /// [`Xso<dyn Trait>`][`Xso`].
    ///
    /// # Iteration order
    ///
    /// Items which have the same concrete type are grouped and their ordering
    /// with respect to one another is preserved. However, the ordering of
    /// items with *different* concrete types is unspecified.
    ///
    /// # Example
    ///
    /// ```
    #[doc = include_str!("xso_vec_test_prelude.rs")]
    /// #[derive(PartialEq, Debug)]
    /// struct Foo(u8);
    /// impl Trait for Foo {}
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct Bar(u16);
    /// impl Trait for Bar {}
    ///
    /// let mut vec = XsoVec::<dyn Trait>::new();
    /// vec.push(Foo(1));
    /// vec.push(Bar(1));
    /// vec.push(Foo(2));
    ///
    /// for item in vec.iter() {
    ///     println!("{:?}", item);
    /// }
    /// ```
    pub fn iter(&self) -> XsoVecIter<'_, T> {
        XsoVecIter {
            remaining: self.len(),
            outer: self.inner.values(),
            inner: None,
        }
    }

    /// Iterate the items inside the container, mutably.
    ///
    /// This iterator (unlike the iterator returned by
    /// [`iter_typed_mut()`][`Self::iter_typed_mut`]) yields mutable
    /// references to **untyped** [`Xso<dyn Trait>`][`Xso`].
    ///
    /// Please note the information about iteration order of the `XsoVec`
    /// at [`XsoVec::iter`][`Self::iter`].
    pub fn iter_mut(&mut self) -> XsoVecIterMut<'_, T> {
        XsoVecIterMut {
            remaining: self.len(),
            outer: self.inner.values_mut(),
            inner: None,
        }
    }
}

impl<T: ?Sized> IntoIterator for XsoVec<T> {
    type Item = Xso<T>;
    type IntoIter = XsoVecIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        XsoVecIntoIter {
            remaining: self.len(),
            outer: self.inner.into_values(),
            inner: None,
        }
    }
}

impl<'x, T: ?Sized> IntoIterator for &'x XsoVec<T> {
    type Item = &'x Xso<T>;
    type IntoIter = XsoVecIter<'x, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'x, T: ?Sized> IntoIterator for &'x mut XsoVec<T> {
    type Item = &'x mut Xso<T>;
    type IntoIter = XsoVecIterMut<'x, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: DynXso + ?Sized + 'static> Extend<Xso<T>> for XsoVec<T> {
    fn extend<I: IntoIterator<Item = Xso<T>>>(&mut self, iter: I) {
        for item in iter {
            self.ensure_vec_mut_for(item.inner_type_id()).push(item);
        }
    }
}

/// Helper types for [`XsoVec`].
pub mod xso_vec {
    use super::*;

    /// Iterator over the contents of an [`XsoVec`].
    pub struct XsoVecIter<'x, T: ?Sized> {
        pub(super) outer: btree_map::Values<'x, TypeId, Vec<Xso<T>>>,
        pub(super) inner: Option<slice::Iter<'x, Xso<T>>>,
        pub(super) remaining: usize,
    }

    impl<'x, T: ?Sized> Iterator for XsoVecIter<'x, T> {
        type Item = &'x Xso<T>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if let Some(inner) = self.inner.as_mut() {
                    if let Some(item) = inner.next() {
                        self.remaining = self.remaining.saturating_sub(1);
                        return Some(item);
                    }
                    // Inner is exhausted, so equivalent to None, fall through.
                }
                // The `?` in there is our exit condition.
                self.inner = Some(self.outer.next()?.deref().iter())
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.remaining, Some(self.remaining))
        }
    }

    /// Mutable iterator over the contents of an [`XsoVec`].
    pub struct XsoVecIterMut<'x, T: ?Sized> {
        pub(super) outer: btree_map::ValuesMut<'x, TypeId, Vec<Xso<T>>>,
        pub(super) inner: Option<slice::IterMut<'x, Xso<T>>>,
        pub(super) remaining: usize,
    }

    impl<'x, T: ?Sized> Iterator for XsoVecIterMut<'x, T> {
        type Item = &'x mut Xso<T>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if let Some(inner) = self.inner.as_mut() {
                    if let Some(item) = inner.next() {
                        self.remaining = self.remaining.saturating_sub(1);
                        return Some(item);
                    }
                    // Inner is exhausted, so equivalent to None, fall through.
                }
                // The `?` in there is our exit condition.
                self.inner = Some(self.outer.next()?.deref_mut().iter_mut())
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.remaining, Some(self.remaining))
        }
    }

    /// Iterator over the owned contents of an [`XsoVec`].
    pub struct XsoVecIntoIter<T: ?Sized> {
        pub(super) outer: btree_map::IntoValues<TypeId, Vec<Xso<T>>>,
        pub(super) inner: Option<vec::IntoIter<Xso<T>>>,
        pub(super) remaining: usize,
    }

    impl<T: ?Sized> Iterator for XsoVecIntoIter<T> {
        type Item = Xso<T>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if let Some(inner) = self.inner.as_mut() {
                    if let Some(item) = inner.next() {
                        self.remaining = self.remaining.saturating_sub(1);
                        return Some(item);
                    }
                    // Inner is exhausted, so equivalent to None, fall through.
                }
                // The `?` in there is our exit condition.
                self.inner = Some(self.outer.next()?.into_iter())
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.remaining, Some(self.remaining))
        }
    }
}

use xso_vec::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xso_inner_type_id_is_correct() {
        trait Trait: Any {}
        crate::derive_dyn_traits!(Trait use () = ());
        struct Foo;
        impl Trait for Foo {}

        let ty_id = TypeId::of::<Foo>();
        let x: Xso<dyn Trait> = Xso::wrap(Foo);
        assert_eq!(x.inner_type_id(), ty_id);
    }
}
