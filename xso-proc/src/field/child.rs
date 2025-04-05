// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module concerns the processing of typed child elements.
//!
//! In particular, it provides both `#[xml(extract)]` and `#[xml(child)]`
//! implementations in a single type.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, *};

use crate::compound::Compound;
use crate::error_message::{self, ParentRef};
use crate::meta::{AmountConstraint, Flag, NameRef, NamespaceRef};
use crate::scope::{AsItemsScope, FromEventsScope};
use crate::types::{
    as_xml_iter_fn, default_fn, extend_fn, from_events_fn, from_xml_builder_ty,
    into_iterator_into_iter_fn, into_iterator_item_ty, into_iterator_iter_ty, item_iter_ty,
    option_as_xml_ty, option_ty, ref_ty, ty_from_ident,
};

use super::{Field, FieldBuilderPart, FieldIteratorPart, FieldTempInit, NestedMatcher};

/// The field maps to a child
pub(super) struct ChildField {
    /// Flag indicating whether the value should be defaulted if the
    /// child is absent.
    pub(super) default_: Flag,

    /// Number of child elements allowed.
    pub(super) amount: AmountConstraint,

    /// If set, the child element is not parsed as a field implementing
    /// `FromXml` / `AsXml`, but instead its contents are extracted.
    pub(super) extract: Option<ExtractDef>,
}

impl Field for ChildField {
    fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldBuilderPart> {
        let (element_ty, is_container) = match self.amount {
            AmountConstraint::FixedSingle(_) => (ty.clone(), false),
            AmountConstraint::Any(_) => (into_iterator_item_ty(ty.clone()), true),
        };

        let (extra_defs, matcher, fetch, builder) = match self.extract {
            Some(ref extract) => extract.make_from_xml_builder_parts(
                scope,
                container_name,
                member,
                is_container,
                ty,
            )?,
            None => {
                let FromEventsScope {
                    ref substate_result,
                    ..
                } = scope;

                let from_events = from_events_fn(element_ty.clone());

                let matcher = quote! { #from_events(name, attrs) };
                let builder = from_xml_builder_ty(element_ty.clone());

                (
                    TokenStream::default(),
                    matcher,
                    quote! { #substate_result },
                    builder,
                )
            }
        };

        let field_access = scope.access_field(member);
        match self.amount {
            AmountConstraint::FixedSingle(_) => {
                let missing_msg = error_message::on_missing_child(container_name, member);
                let duplicate_msg = error_message::on_duplicate_child(container_name, member);

                let on_absent = match self.default_ {
                    Flag::Absent => quote! {
                        return ::core::result::Result::Err(::xso::error::Error::Other(#missing_msg).into())
                    },
                    Flag::Present(_) => {
                        let default_ = default_fn(element_ty.clone());
                        quote! {
                            #default_()
                        }
                    }
                };

                Ok(FieldBuilderPart::Nested {
                    extra_defs,
                    value: FieldTempInit {
                        init: quote! { ::core::option::Option::None },
                        ty: option_ty(ty.clone()),
                    },
                    matcher: NestedMatcher::Selective(quote! {
                        match #matcher {
                            ::core::result::Result::Ok(v) => if #field_access.is_some() {
                                ::core::result::Result::Err(::xso::error::FromEventsError::Invalid(::xso::error::Error::Other(#duplicate_msg)))
                            } else {
                                ::core::result::Result::Ok(v)
                            },
                            ::core::result::Result::Err(e) => ::core::result::Result::Err(e),
                        }
                    }),
                    builder,
                    collect: quote! {
                        #field_access = ::core::option::Option::Some(#fetch);
                    },
                    finalize: quote! {
                        match #field_access {
                            ::core::option::Option::Some(value) => value,
                            ::core::option::Option::None => #on_absent,
                        }
                    },
                })
            }
            AmountConstraint::Any(_) => {
                let ty_extend = extend_fn(ty.clone(), element_ty.clone());
                let ty_default = default_fn(ty.clone());
                Ok(FieldBuilderPart::Nested {
                    extra_defs,
                    value: FieldTempInit {
                        init: quote! { #ty_default() },
                        ty: ty.clone(),
                    },
                    matcher: NestedMatcher::Selective(matcher),
                    builder,
                    collect: quote! {
                        #ty_extend(&mut #field_access, [#fetch]);
                    },
                    finalize: quote! { #field_access },
                })
            }
        }
    }

    fn make_iterator_part(
        &self,
        scope: &AsItemsScope,
        container_name: &ParentRef,
        bound_name: &Ident,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldIteratorPart> {
        let AsItemsScope { ref lifetime, .. } = scope;

        let (item_ty, is_container) = match self.amount {
            AmountConstraint::FixedSingle(_) => (ty.clone(), false),
            AmountConstraint::Any(_) => {
                // This should give us the type of element stored in the
                // collection.
                (into_iterator_item_ty(ty.clone()), true)
            }
        };

        let (extra_defs, init, iter_ty) = match self.extract {
            Some(ref extract) => extract.make_as_item_iter_parts(
                scope,
                ty,
                container_name,
                bound_name,
                member,
                is_container,
            )?,
            None => {
                let as_xml_iter = as_xml_iter_fn(item_ty.clone());
                let item_iter = item_iter_ty(item_ty.clone(), lifetime.clone());

                (
                    TokenStream::default(),
                    quote! { #as_xml_iter(#bound_name)? },
                    item_iter,
                )
            }
        };

        match self.amount {
            AmountConstraint::FixedSingle(_) => Ok(FieldIteratorPart::Content {
                extra_defs,
                value: FieldTempInit { init, ty: iter_ty },
                generator: quote! {
                    #bound_name.next().transpose()
                },
            }),
            AmountConstraint::Any(_) => {
                // This is the collection type we actually work
                // with -- as_xml_iter uses references after all.
                let ty = ref_ty(ty.clone(), lifetime.clone());

                // But the iterator for iterating over the elements
                // inside the collection must use the ref type.
                let element_iter = into_iterator_iter_ty(ty.clone());

                // And likewise the into_iter impl.
                let into_iter = into_iterator_into_iter_fn(ty.clone());

                let state_ty = Type::Tuple(TypeTuple {
                    paren_token: token::Paren::default(),
                    elems: [element_iter, option_ty(iter_ty)].into_iter().collect(),
                });

                Ok(FieldIteratorPart::Content {
                    extra_defs,
                    value: FieldTempInit {
                        init: quote! {
                            (#into_iter(#bound_name), ::core::option::Option::None)
                        },
                        ty: state_ty,
                    },
                    generator: quote! {
                        loop {
                            if let ::core::option::Option::Some(current) = #bound_name.1.as_mut() {
                                if let ::core::option::Option::Some(item) = current.next() {
                                    break ::core::option::Option::Some(item).transpose();
                                }
                            }
                            if let ::core::option::Option::Some(item) = #bound_name.0.next() {
                                #bound_name.1 = ::core::option::Option::Some({
                                    let #bound_name = item;
                                    #init
                                });
                            } else {
                                break ::core::result::Result::Ok(::core::option::Option::None)
                            }
                        }
                    },
                })
            }
        }
    }
}

/// Definition of what to extract from a child element.
pub(super) struct ExtractDef {
    /// The XML namespace of the child to extract data from.
    pub(super) xml_namespace: NamespaceRef,

    /// The XML name of the child to extract data from.
    pub(super) xml_name: NameRef,

    /// Compound which contains the arguments of the `extract(..)` meta
    /// (except the `from`), transformed into a struct with unnamed
    /// fields.
    ///
    /// This is used to generate the parsing/serialisation code, by
    /// essentially "declaring" a shim struct, as if it were a real Rust
    /// struct, and using the result of the parsing process directly for
    /// the field on which the `extract(..)` option was used, instead of
    /// putting it into a Rust struct.
    pub(super) parts: Compound,
}

impl ExtractDef {
    /// Construct
    /// [`FieldBuilderPart::Nested::extra_defs`],
    /// [`FieldBuilderPart::Nested::matcher`],
    /// an expression which pulls the extraction result from
    /// `substate_result`,
    /// and the [`FieldBuilderPart::Nested::builder`] type.
    fn make_from_xml_builder_parts(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
        member: &Member,
        collecting_into_container: bool,
        output_ty: &Type,
    ) -> Result<(TokenStream, TokenStream, TokenStream, Type)> {
        let FromEventsScope {
            ref substate_result,
            ..
        } = scope;

        let xml_namespace = &self.xml_namespace;
        let xml_name = &self.xml_name;

        let from_xml_builder_ty_ident = scope.make_member_type_name(member, "FromXmlBuilder");
        let state_ty_ident = quote::format_ident!("{}State", from_xml_builder_ty_ident,);

        let extra_defs = self.parts.make_from_events_statemachine(
            &state_ty_ident,
            &container_name.child(member.clone()),
            "",
        )?.with_augmented_init(|init| quote! {
            if name.0 == #xml_namespace && name.1 == #xml_name {
                #init
            } else {
                ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch { name, attrs })
            }
        }).compile().render(
            &Visibility::Inherited,
            &from_xml_builder_ty_ident,
            &state_ty_ident,
            &self.parts.to_tuple_ty().into(),
            None,
        )?;
        let from_xml_builder_ty = ty_from_ident(from_xml_builder_ty_ident.clone()).into();

        let matcher = quote! { #state_ty_ident::new(name, attrs).map(|x| #from_xml_builder_ty_ident(::core::option::Option::Some(x))) };

        let inner_ty = self.parts.to_single_or_tuple_ty();

        let fetch = if self.parts.field_count() == 1 {
            // If we extract only a single field, we automatically unwrap the
            // tuple, because that behaviour is more obvious to users.
            quote! { #substate_result.0 }
        } else {
            // If we extract more than one field, we pass the value down as
            // the tuple that it is.
            quote! { #substate_result }
        };

        let fetch = if collecting_into_container {
            // This is for symmetry with the AsXml implementation part. Please
            // see there for why we cannot do option magic in the collection
            // case.
            fetch
        } else {
            // This little ".into()" here goes a long way. It relies on one of
            // the most underrated trait implementations in the standard
            // library: `impl From<T> for Option<T>`, which creates a
            // `Some(_)` from a `T`. Why is it so great? Because there is also
            // `impl From<Option<T>> for Option<T>` (obviously), which is just
            // a move. So even without knowing the exact type of the substate
            // result and the field, we can make an "downcast" to `Option<T>`
            // if the field is of type `Option<T>`, and it does the right
            // thing no matter whether the extracted field is of type
            // `Option<T>` or `T`.
            //
            // And then, type inference does the rest: There is ambiguity
            // there, of course, if we call `.into()` on a value of type
            // `Option<T>`: Should Rust wrap it into another layer of
            // `Option`, or should it just move the value? The answer lies in
            // the type constraint imposed by the place the value is *used*,
            // which is strictly bound by the field's type (so there is, in
            // fact, no ambiguity). So this works all kinds of magic.
            quote_spanned! {
                output_ty.span()=>
                    <#output_ty as ::core::convert::From::<#inner_ty>>::from(#fetch)
            }
        };

        Ok((extra_defs, matcher, fetch, from_xml_builder_ty))
    }

    /// Construct
    /// [`FieldIteratorPart::Content::extra_defs`],
    /// the [`FieldIteratorPart::Content::value`] init,
    /// and the iterator type.
    fn make_as_item_iter_parts(
        &self,
        scope: &AsItemsScope,
        input_ty: &Type,
        container_name: &ParentRef,
        bound_name: &Ident,
        member: &Member,
        iterating_container: bool,
    ) -> Result<(TokenStream, TokenStream, Type)> {
        let AsItemsScope { ref lifetime, .. } = scope;

        let xml_namespace = &self.xml_namespace;
        let xml_name = &self.xml_name;

        let item_iter_ty_ident = scope.make_member_type_name(member, "AsXmlIterator");
        let state_ty_ident = quote::format_ident!("{}State", item_iter_ty_ident,);
        let mut item_iter_ty = ty_from_ident(item_iter_ty_ident.clone());
        item_iter_ty.path.segments[0].arguments =
            PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                colon2_token: None,
                lt_token: token::Lt::default(),
                args: [GenericArgument::Lifetime(lifetime.clone())]
                    .into_iter()
                    .collect(),
                gt_token: token::Gt::default(),
            });
        let item_iter_ty = item_iter_ty.into();
        let tuple_ty = self.parts.to_ref_tuple_ty(lifetime);

        let (repack, inner_ty) = match self.parts.single_ty() {
            Some(single_ty) => (
                quote! { #bound_name, },
                ref_ty(single_ty.clone(), lifetime.clone()),
            ),
            None => {
                let mut repack_tuple = TokenStream::default();
                // The cast here is sound, because the constructor of Compound
                // already asserts that there are less than 2^32 fields (with
                // what I think is a great error message, go check it out).
                for i in 0..(tuple_ty.elems.len() as u32) {
                    let index = Index {
                        index: i,
                        span: Span::call_site(),
                    };
                    repack_tuple.extend(quote! {
                        &#bound_name.#index,
                    })
                }
                let ref_tuple_ty = ref_ty(self.parts.to_tuple_ty().into(), lifetime.clone());
                (repack_tuple, ref_tuple_ty)
            }
        };

        let extra_defs = self
            .parts
            .make_as_item_iter_statemachine(
                &container_name.child(member.clone()),
                &state_ty_ident,
                "",
                lifetime,
            )?
            .with_augmented_init(|init| {
                quote! {
                    let name = (
                        ::xso::exports::rxml::Namespace::from(#xml_namespace),
                        ::xso::exports::alloc::borrow::Cow::Borrowed(#xml_name),
                    );
                    #init
                }
            })
            .compile()
            .render(
                &Visibility::Inherited,
                &tuple_ty.into(),
                &state_ty_ident,
                lifetime,
                &item_iter_ty,
            )?;

        let (make_iter, item_iter_ty) = if iterating_container {
            // When we are iterating a container, the container's iterator's
            // item type may either be `&(A, B, ...)` or `(&A, &B, ...)`.
            // Unfortunately, to be able to handle both, we need to omit the
            // magic Option cast, because we cannot specify the type of the
            // argument of the `.map(...)` closure and rust is not able to
            // infer that type because the repacking is too opaque.
            //
            // However, this is not much of a loss, because it doesn't really
            // make sense to have the option cast there, anyway: optional
            // elements in a container would be weird.
            (
                quote! {
                    #item_iter_ty_ident::new((#repack))?
                },
                item_iter_ty,
            )
        } else {
            // Again we exploit the extreme usefulness of the
            // `impl From<T> for Option<T>`. We already wrote extensively
            // about that in [`make_from_xml_builder_parts`] implementation
            // corresponding to this code above, and we will not repeat it
            // here.

            // These sections with quote_spanned are used to improve error
            // messages on type mismatches. Without these, the rustc errors
            // will point at `#[derive(AsXml)]` only, instead of directly
            // pointing at the sources of those types.
            let cast = quote_spanned! { input_ty.span()=>
                ::core::option::Option::from(#bound_name)
            };
            let type_assert = quote_spanned! { inner_ty.span()=>
                ::core::option::Option<#inner_ty>
            };
            (
                quote! {
                    ::xso::asxml::OptionAsXml::new({ let x: #type_assert = #cast; x.map(|#bound_name: #inner_ty| {
                        #item_iter_ty_ident::new((#repack))
                    })}.transpose()?)
                },
                option_as_xml_ty(item_iter_ty),
            )
        };

        Ok((extra_defs, make_iter, item_iter_ty))
    }
}
