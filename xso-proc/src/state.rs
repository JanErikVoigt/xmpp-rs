// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! State machines for parsing and serialising of structs and enums.

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::*;

/// A single state in a parser or serializer state machine.
pub(crate) struct State {
    /// Name of the state enum variant for this state.
    name: Ident,

    /// Declaration of members of the state enum in this state.
    decl: TokenStream,

    /// Destructuring of members of the state enum in this state.
    destructure: TokenStream,

    /// Right-hand-side of the match arm for this state.
    advance_body: TokenStream,

    /// If set, that identifier will be bound mutably.
    uses_mut: Option<Ident>,
}

impl State {
    /// Create a new state with the a builder data field.
    ///
    /// This is a convenience wrapper around `new()` and `add_field()`. This
    /// wrapper, or its equivalent, **must** be used for states used in
    /// [`FromEventsStateMachine`] state machines, as those expect that the
    /// first field is the builder data at render time.
    pub(crate) fn new_with_builder(
        name: Ident,
        builder_data_ident: &Ident,
        builder_data_ty: &Type,
    ) -> Self {
        let mut result = Self::new(name);
        result.add_field(builder_data_ident, builder_data_ty);
        result
    }

    /// Create a new, empty state.
    ///
    /// Note that an empty state will generate invalid code. At the very
    /// least, a body must be added using [`Self::set_impl`] or
    /// [`Self::with_impl`]. The various state machines may also have
    /// additional requirements.
    pub(crate) fn new(name: Ident) -> Self {
        Self {
            name,
            decl: TokenStream::default(),
            destructure: TokenStream::default(),
            advance_body: TokenStream::default(),
            uses_mut: None,
        }
    }

    /// Add a field to this state's data.
    ///
    /// - `name` is the name under which the data will be accessible in the
    ///   state's implementation.
    /// - `ty` must be the data field's type.
    pub(crate) fn add_field(&mut self, name: &Ident, ty: &Type) {
        self.decl.extend(quote! { #name: #ty, });
        self.destructure.extend(quote! { #name, });
    }

    /// Modify the state to include another field and return the modified
    /// state.
    ///
    /// This is a consume-and-return-style version of [`Self::add_field`].
    pub(crate) fn with_field(mut self, name: &Ident, ty: &Type) -> Self {
        self.add_field(name, ty);
        self
    }

    /// Set the `advance` implementation of this state.
    ///
    /// `body` must be the body of the right hand side of the match arm for
    /// the `advance` implementation of the state machine.
    ///
    /// See [`FromEventsStateMachine::advance_match_arms`] and
    /// [`AsItemsSubmachine::compile`] for the respective
    /// requirements on the implementations.
    pub(crate) fn with_impl(mut self, body: TokenStream) -> Self {
        self.advance_body = body;
        self
    }

    /// Override the current `advance` implementation of this state.
    ///
    /// This is an in-place version of [`Self::with_impl`].
    pub(crate) fn set_impl(&mut self, body: TokenStream) {
        self.advance_body = body;
    }

    /// Modify the state to mark the given field as mutable and return the
    /// modified state.
    pub(crate) fn with_mut(mut self, ident: &Ident) -> Self {
        assert!(self.uses_mut.is_none());
        self.uses_mut = Some(ident.clone());
        self
    }
}

/// A partial [`FromEventsStateMachine`] which only covers the builder for a
/// single compound.
///
/// See [`FromEventsStateMachine`] for more information on the state machines
/// in general.
pub(crate) struct FromEventsSubmachine {
    /// Additional items necessary for the statemachine.
    pub(crate) defs: TokenStream,

    /// States and state transition implementations.
    pub(crate) states: Vec<State>,

    /// Initializer expression.
    ///
    /// This expression must evaluate to a
    /// `Result<#state_ty_ident, xso::FromEventsError>`.
    pub(crate) init: TokenStream,
}

impl FromEventsSubmachine {
    /// Convert a partial state machine into a full state machine.
    ///
    /// This converts the abstract [`State`] items into token
    /// streams for the respective parts of the state machine (the state
    /// definitions and the match arms), rendering them effectively immutable.
    pub(crate) fn compile(self) -> FromEventsStateMachine {
        let mut state_defs = TokenStream::default();
        let mut advance_match_arms = TokenStream::default();

        for state in self.states {
            let State {
                name,
                decl,
                destructure,
                advance_body,
                uses_mut,
            } = state;

            state_defs.extend(quote! {
                #name { #decl },
            });

            let binding = if let Some(uses_mut) = uses_mut.as_ref() {
                quote! {
                    let mut #uses_mut = #uses_mut;
                }
            } else {
                TokenStream::default()
            };

            // XXX: nasty hack, but works: the first member of the enum always
            // exists and it always is the builder data, which we always need
            // mutably available. So we can just prefix the destructuring
            // token stream with `mut` to make that first member mutable.
            advance_match_arms.extend(quote! {
                Self::#name { mut #destructure } => {
                    #binding
                    #advance_body
                }
            });
        }

        FromEventsStateMachine {
            defs: self.defs,
            state_defs,
            advance_match_arms,
            variants: vec![FromEventsEntryPoint { init: self.init }],
            pre_init: TokenStream::default(),
            fallback: None,
        }
    }

    /// Update the [`init`][`Self::init`] field in-place.
    ///
    /// The function will receive a reference to the current `init` value,
    /// allowing to create "wrappers" around that existing code.
    pub(crate) fn with_augmented_init<F: FnOnce(&TokenStream) -> TokenStream>(
        mut self,
        f: F,
    ) -> Self {
        let new_init = f(&self.init);
        self.init = new_init;
        self
    }
}

/// A partial [`AsItemsStateMachine`] which only covers the builder for a
/// single compound.
///
/// See [`AsItemsStateMachine`] for more information on the state machines
/// in general.
pub(crate) struct AsItemsSubmachine {
    /// Additional items necessary for the statemachine.
    pub(crate) defs: TokenStream,

    /// States and state transition implementations.
    pub(crate) states: Vec<State>,

    /// A pattern match which destructures the target type into its parts, for
    /// use by `init`.
    pub(crate) destructure: TokenStream,

    /// An expression which uses the names bound in `destructure` to create a
    /// an instance of the state enum.
    ///
    /// The state enum type is available as `Self` in that context.
    pub(crate) init: TokenStream,
}

impl AsItemsSubmachine {
    /// Convert a partial state machine into a full state machine.
    ///
    /// This converts the abstract [`State`] items into token
    /// streams for the respective parts of the state machine (the state
    /// definitions and the match arms), rendering them effectively immutable.
    ///
    /// This requires that the [`State::advance_body`] token streams evaluate
    /// to an `Option<Item>`. If it evaluates to `Some(.)`, that is
    /// emitted from the iterator. If it evaluates to `None`, the `advance`
    /// implementation is called again.
    ///
    /// Each state implementation is augmented to also enter the next state,
    /// causing the iterator to terminate eventually.
    pub(crate) fn compile(self) -> AsItemsStateMachine {
        let mut state_defs = TokenStream::default();
        let mut advance_match_arms = TokenStream::default();

        for (i, state) in self.states.iter().enumerate() {
            let State {
                ref name,
                ref decl,
                ref destructure,
                ref advance_body,
                ref uses_mut,
            } = state;

            let footer = match self.states.get(i + 1) {
                Some(State {
                    name: ref next_name,
                    destructure: ref construct_next,
                    ..
                }) => {
                    quote! {
                        ::core::result::Result::Ok((::core::option::Option::Some(Self::#next_name { #construct_next }), item))
                    }
                }
                // final state -> exit the state machine
                None => {
                    quote! {
                        ::core::result::Result::Ok((::core::option::Option::None, item))
                    }
                }
            };

            state_defs.extend(quote! {
                #name { #decl },
            });

            if let Some(uses_mut) = uses_mut.as_ref() {
                // the variant is non-consuming, meaning it can be called
                // multiple times and it uses the identifier in `uses_mut`
                // mutably.
                // the transition is only triggered when it emits a None
                // item
                // (we cannot do this at the place the `State` is constructed,
                // because we don't yet know all its fields then; it must be
                // done here.)
                advance_match_arms.extend(quote! {
                    Self::#name { #destructure } => {
                        let mut #uses_mut = #uses_mut;
                        match #advance_body {
                            ::core::option::Option::Some(item) => {
                                ::core::result::Result::Ok((::core::option::Option::Some(Self::#name { #destructure }), ::core::option::Option::Some(item)))
                            },
                            item => { #footer },
                        }
                    }
                });
            } else {
                // if the variant is consuming, it can only be called once.
                // it may or may not emit an event, but the transition is
                // always triggered
                advance_match_arms.extend(quote! {
                    Self::#name { #destructure } => {
                        let item = #advance_body;
                        #footer
                    }
                });
            }
        }

        AsItemsStateMachine {
            defs: self.defs,
            state_defs,
            advance_match_arms,
            variants: vec![AsItemsEntryPoint {
                init: self.init,
                destructure: self.destructure,
            }],
        }
    }

    /// Update the [`init`][`Self::init`] field in-place.
    ///
    /// The function will receive a reference to the current `init` value,
    /// allowing to create "wrappers" around that existing code.
    pub(crate) fn with_augmented_init<F: FnOnce(&TokenStream) -> TokenStream>(
        mut self,
        f: F,
    ) -> Self {
        let new_init = f(&self.init);
        self.init = new_init;
        self
    }
}

/// Container for a single entrypoint into a [`FromEventsStateMachine`].
pub(crate) struct FromEventsEntryPoint {
    pub(crate) init: TokenStream,
}

/// A single variant's entrypoint into the event iterator.
pub(crate) struct AsItemsEntryPoint {
    /// A pattern match which destructures the target type into its parts, for
    /// use by `init`.
    destructure: TokenStream,

    /// An expression which uses the names bound in `destructure` to create a
    /// an instance of the state enum.
    ///
    /// The state enum type is available as `Self` in that context.
    init: TokenStream,
}

/// # State machine to implement `xso::FromEventsBuilder`
///
/// This struct represents a state machine consisting of the following parts:
///
/// - Extra dependencies ([`Self::defs`])
/// - States ([`Self::state_defs`])
/// - Transitions ([`Self::advance_match_arms`])
/// - Entrypoints ([`Self::variants`])
///
/// Such a state machine is best constructed by constructing one or
/// more [`FromEventsSubmachine`] structs and converting/merging them using
/// `into()` and [`merge`][`Self::merge`].
///
/// A state machine has an output type (corresponding to
/// `xso::FromEventsBuilder::Output`), which is however only implicitly defined
/// by the expressions generated in the `advance_match_arms`. That means that
/// merging submachines with different output types works, but will then generate
/// code which will fail to compile.
///
/// When converted to Rust code, the state machine will manifest as (among other
/// things) an enum type which contains all states and which has an `advance`
/// method. That method consumes the enum value and returns either a new enum
/// value, an error, or the output type of the state machine.
#[derive(Default)]
pub(crate) struct FromEventsStateMachine {
    /// Extra items which are needed for the state machine implementation.
    defs: TokenStream,

    /// Extra code run during pre-init phase.
    pre_init: TokenStream,

    /// Code to run as fallback if none of the branches matched the start
    /// event.
    ///
    /// If absent, a `FromEventsError::Mismatch` is generated.
    fallback: Option<TokenStream>,

    /// A sequence of enum variant declarations, separated and terminated by
    /// commas.
    state_defs: TokenStream,

    /// A sequence of `match self { .. }` arms, where `self` is the state
    /// enumeration type.
    ///
    /// Each match arm must either diverge or evaluate to a
    /// `Result<ControlFlow<State, Output>, xso::error::Error>`, where `State`
    /// is the state enumeration and `Output` is the state machine's output
    /// type.
    advance_match_arms: TokenStream,

    /// The different entrypoints for the state machine.
    ///
    /// This may only contain more than one element if an enumeration is being
    /// constructed by the resulting state machine.
    variants: Vec<FromEventsEntryPoint>,
}

impl FromEventsStateMachine {
    /// Create a new, empty state machine.
    pub(crate) fn new() -> Self {
        Self {
            defs: TokenStream::default(),
            state_defs: TokenStream::default(),
            advance_match_arms: TokenStream::default(),
            pre_init: TokenStream::default(),
            variants: Vec::new(),
            fallback: None,
        }
    }

    /// Merge another state machine into this state machine.
    ///
    /// This *discards* the other state machine's pre-init code.
    pub(crate) fn merge(&mut self, other: FromEventsStateMachine) {
        assert!(other.fallback.is_none());
        self.defs.extend(other.defs);
        self.state_defs.extend(other.state_defs);
        self.advance_match_arms.extend(other.advance_match_arms);
        self.variants.extend(other.variants);
    }

    /// Set additional code to inject at the head of the `new` method for the
    /// builder.
    ///
    /// This can be used to do preliminary checks and is commonly used with
    /// specifically-formed init codes on the variants.
    pub(crate) fn set_pre_init(&mut self, code: TokenStream) {
        self.pre_init = code;
    }

    /// Set the fallback code to use if none of the branches matches the start
    /// event.
    ///
    /// By default, a `FromEventsError::Mismatch` is generated.
    pub(crate) fn set_fallback(&mut self, code: TokenStream) {
        self.fallback = Some(code);
    }

    /// Render the state machine as a token stream.
    ///
    /// The token stream contains the following pieces:
    /// - Any definitions necessary for the statemachine to operate
    /// - The state enum
    /// - The builder struct
    /// - The `xso::FromEventsBuilder` impl on the builder struct
    /// - A `fn new(rxml::QName, rxml::AttrMap) -> Result<Self>` on the
    ///   builder struct.
    pub(crate) fn render(
        self,
        vis: &Visibility,
        builder_ty_ident: &Ident,
        state_ty_ident: &Ident,
        output_ty: &Type,
        validate_fn: Option<&Path>,
    ) -> Result<TokenStream> {
        let Self {
            defs,
            state_defs,
            advance_match_arms,
            variants,
            pre_init,
            fallback,
        } = self;

        let mut init_body = pre_init;
        for variant in variants {
            let FromEventsEntryPoint { init } = variant;
            init_body.extend(quote! {
                let (name, mut attrs) = match { { let _ = &mut attrs; } #init } {
                    ::core::result::Result::Ok(v) => return ::core::result::Result::Ok(v),
                    ::core::result::Result::Err(::xso::error::FromEventsError::Invalid(e)) => return ::core::result::Result::Err(::xso::error::FromEventsError::Invalid(e)),
                    ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch { name, attrs }) => (name, attrs),
                };
            })
        }

        let fallback = fallback.unwrap_or_else(|| {
            quote! {
                ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch { name, attrs })
            }
        });

        let output_ty_ref = make_ty_ref(output_ty);

        let docstr = format!("Build a {0} from XML events.\n\nThis type is generated using the [`macro@xso::FromXml`] derive macro and implements [`xso::FromEventsBuilder`] for {0}.", output_ty_ref);

        let validate_call = match validate_fn {
            None => quote! {
                // needed to avoid unused_mut warning.
                let _ = &mut value;
            },
            Some(validate_fn) => {
                quote! {
                    #validate_fn(&mut value)?;
                }
            }
        };

        Ok(quote! {
            #defs

            enum #state_ty_ident {
                #state_defs
            }

            impl #state_ty_ident {
                fn advance(mut self, ev: ::xso::exports::rxml::Event) -> ::core::result::Result<::core::ops::ControlFlow<Self, #output_ty>, ::xso::error::Error> {
                    match self {
                        #advance_match_arms
                    }.and_then(|__ok| {
                        match __ok {
                            ::core::ops::ControlFlow::Break(st) => ::core::result::Result::Ok(::core::ops::ControlFlow::Break(st)),
                            ::core::ops::ControlFlow::Continue(result) => {
                                ::core::result::Result::Ok(::core::ops::ControlFlow::Continue(result))
                            }
                        }
                    })
                }
            }

            impl #builder_ty_ident {
                fn new(
                    name: ::xso::exports::rxml::QName,
                    attrs: ::xso::exports::rxml::AttrMap,
                ) -> ::core::result::Result<Self, ::xso::error::FromEventsError> {
                    #state_ty_ident::new(name, attrs).map(|ok| Self(::core::option::Option::Some(ok)))
                }
            }

            #[doc = #docstr]
            #[doc(hidden)]
            #vis struct #builder_ty_ident(::core::option::Option<#state_ty_ident>);

            impl ::xso::FromEventsBuilder for #builder_ty_ident {
                type Output = #output_ty;

                fn feed(&mut self, ev: ::xso::exports::rxml::Event) -> ::core::result::Result<::core::option::Option<Self::Output>, ::xso::error::Error> {
                    let inner = self.0.take().expect("feed called after completion");
                    match inner.advance(ev)? {
                        ::core::ops::ControlFlow::Continue(mut value) => {
                            #validate_call
                            ::core::result::Result::Ok(::core::option::Option::Some(value))
                        }
                        ::core::ops::ControlFlow::Break(st) => {
                            self.0 = ::core::option::Option::Some(st);
                            ::core::result::Result::Ok(::core::option::Option::None)
                        }
                    }
                }
            }

            impl #state_ty_ident {
                fn new(
                    name: ::xso::exports::rxml::QName,
                    mut attrs: ::xso::exports::rxml::AttrMap,
                ) -> ::core::result::Result<Self, ::xso::error::FromEventsError> {
                    #init_body
                    { let _ = &mut attrs; }
                    #fallback
                }
            }
        })
    }
}

/// # State machine to implement an `Iterator<Item = rxml::Event>`.
///
/// This struct represents a state machine consisting of the following parts:
///
/// - Extra dependencies ([`Self::defs`])
/// - States ([`Self::state_defs`])
/// - Transitions ([`Self::advance_match_arms`])
/// - Entrypoints ([`Self::variants`])
///
/// Such a state machine is best constructed by constructing one or
/// more [`FromEventsSubmachine`] structs and converting/merging them using
/// `into()` and [`merge`][`Self::merge`].
///
/// A state machine has an output type (corresponding to
/// `xso::FromEventsBuilder::Output`), which is however only implicitly defined
/// by the expressions generated in the `advance_match_arms`. That means that
/// merging submachines with different output types works, but will then generate
/// code which will fail to compile.
///
/// When converted to Rust code, the state machine will manifest as (among other
/// things) an enum type which contains all states and which has an `advance`
/// method. That method consumes the enum value and returns either a new enum
/// value, an error, or the output type of the state machine.
#[derive(Default)]
pub(crate) struct AsItemsStateMachine {
    /// Extra items which are needed for the state machine implementation.
    defs: TokenStream,

    /// A sequence of enum variant declarations, separated and terminated by
    /// commas.
    state_defs: TokenStream,

    /// A sequence of `match self { .. }` arms, where `self` is the state
    /// enumeration type.
    ///
    /// Each match arm must either diverge or evaluate to a
    /// `Result<(Option<State>, Option<Item>), xso::error::Error>`, where
    /// where `State` is the state enumeration.
    ///
    /// If `Some(.)` is returned for the event, that event is emitted. If
    /// `None` is returned for the event, the advance implementation is called
    /// again after switching to the state returned in the `Option<State>`
    /// field.
    ///
    /// If `None` is returned for the `Option<State>`, the iterator
    /// terminates yielding the `Option<Item>` value directly (even if it is
    /// `None`). After the iterator has terminated, it yields `None`
    /// indefinitely.
    advance_match_arms: TokenStream,

    /// The different entrypoints for the state machine.
    ///
    /// This may only contain more than one element if an enumeration is being
    /// serialised by the resulting state machine.
    variants: Vec<AsItemsEntryPoint>,
}

impl AsItemsStateMachine {
    /// Create a new, empty state machine.
    pub(crate) fn new() -> Self {
        Self {
            defs: TokenStream::default(),
            state_defs: TokenStream::default(),
            advance_match_arms: TokenStream::default(),
            variants: Vec::new(),
        }
    }

    /// Merge another state machine into this state machine.
    pub(crate) fn merge(&mut self, other: AsItemsStateMachine) {
        self.defs.extend(other.defs);
        self.state_defs.extend(other.state_defs);
        self.advance_match_arms.extend(other.advance_match_arms);
        self.variants.extend(other.variants);
    }

    /// Render the state machine as a token stream.
    ///
    /// The token stream contains the following pieces:
    /// - Any definitions necessary for the statemachine to operate
    /// - The state enum
    /// - The iterator struct
    /// - The `Iterator` impl on the builder struct
    /// - A `fn new(T) -> Result<Self>` on the iterator struct.
    pub(crate) fn render(
        self,
        vis: &Visibility,
        input_ty_ref: &Type,
        state_ty_ident: &Ident,
        item_iter_ty_lifetime: &Lifetime,
        item_iter_ty: &Type,
    ) -> Result<TokenStream> {
        let Self {
            defs,
            state_defs,
            advance_match_arms,
            mut variants,
        } = self;

        let input_ty_ref_text = make_ty_ref(input_ty_ref);
        let docstr = format!("Convert a {0} into XML events.\n\nThis type is generated using the [`macro@xso::AsXml`] derive macro and implements [`core::iter:Iterator`] for {0}.", input_ty_ref_text);

        let init_body = if variants.len() == 1 {
            let AsItemsEntryPoint { destructure, init } = variants.remove(0);
            quote! {
                {
                    let #destructure = value;
                    #init
                }
            }
        } else {
            let mut match_arms = TokenStream::default();
            for AsItemsEntryPoint { destructure, init } in variants {
                match_arms.extend(quote! {
                    #destructure => { #init }
                });
            }

            quote! {
                match value {
                    #match_arms
                }
            }
        };

        Ok(quote! {
            #defs

            enum #state_ty_ident<#item_iter_ty_lifetime> {
                #state_defs
            }

            impl<#item_iter_ty_lifetime> #state_ty_ident<#item_iter_ty_lifetime> {
                fn advance(mut self) -> ::core::result::Result<(::core::option::Option<Self>, ::core::option::Option<::xso::Item<#item_iter_ty_lifetime>>), ::xso::error::Error> {
                    match self {
                        #advance_match_arms
                    }
                }

                fn new(
                    value: #input_ty_ref,
                ) -> ::core::result::Result<Self, ::xso::error::Error> {
                    ::core::result::Result::Ok(#init_body)
                }
            }

            #[doc = #docstr]
            #[doc(hidden)]
            #vis struct #item_iter_ty(::core::option::Option<#state_ty_ident<#item_iter_ty_lifetime>>);

            impl<#item_iter_ty_lifetime> ::core::iter::Iterator for #item_iter_ty {
                type Item = ::core::result::Result<::xso::Item<#item_iter_ty_lifetime>, ::xso::error::Error>;

                fn next(&mut self) -> ::core::option::Option<Self::Item> {
                    let mut state = self.0.take()?;
                    loop {
                        let (next_state, item) = match state.advance() {
                            ::core::result::Result::Ok(v) => v,
                            ::core::result::Result::Err(e) => return ::core::option::Option::Some(::core::result::Result::Err(e)),
                        };
                        if let ::core::option::Option::Some(item) = item {
                            self.0 = next_state;
                            return ::core::option::Option::Some(::core::result::Result::Ok(item));
                        }
                        // no event, do we have a state?
                        if let ::core::option::Option::Some(st) = next_state {
                            // we do: try again!
                            state = st;
                            continue;
                        } else {
                            // we don't: end of iterator!
                            self.0 = ::core::option::Option::None;
                            return ::core::option::Option::None;
                        }
                    }
                }
            }

            impl<#item_iter_ty_lifetime> #item_iter_ty {
                fn new(value: #input_ty_ref) -> ::core::result::Result<Self, ::xso::error::Error> {
                    #state_ty_ident::new(value).map(|ok| Self(::core::option::Option::Some(ok)))
                }
            }
        })
    }
}

/// Construct a path for an intradoc link from a given type.
fn doc_link_path(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(ref ty) => {
            let (mut buf, offset) = match ty.qself {
                Some(ref qself) => {
                    let mut buf = doc_link_path(&qself.ty)?;
                    buf.push_str("::");
                    (buf, qself.position)
                }
                None => {
                    let mut buf = String::new();
                    if ty.path.leading_colon.is_some() {
                        buf.push_str("::");
                    }
                    (buf, 0)
                }
            };
            let last = ty.path.segments.len() - 1;
            for i in offset..ty.path.segments.len() {
                let segment = &ty.path.segments[i];
                buf.push_str(&segment.ident.to_string());
                if i < last {
                    buf.push_str("::");
                }
            }
            Some(buf)
        }
        Type::Reference(TypeReference { ref elem, .. }) => doc_link_path(elem),
        _ => None,
    }
}

/// Create a markdown snippet which references the given type as cleanly as
/// possible.
///
/// This is used in documentation generation functions.
///
/// Not all types can be linked to; those which cannot be linked to will
/// simply be wrapped in backticks.
fn make_ty_ref(ty: &Type) -> String {
    match doc_link_path(ty) {
        Some(mut path) => {
            path.reserve(4);
            path.insert_str(0, "[`");
            path.push_str("`]");
            path
        }
        None => format!("`{}`", ty.to_token_stream()),
    }
}
