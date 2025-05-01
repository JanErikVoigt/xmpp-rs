// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Handling of structs

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, *};

use crate::common::{AsXmlParts, FromXmlParts, ItemDef, XmlNameMatcher};
use crate::compound::Compound;
use crate::error_message::ParentRef;
use crate::meta::{reject_key, Flag, NameRef, NamespaceRef, QNameRef, XmlCompoundMeta};
use crate::state::{AsItemsSubmachine, FromEventsSubmachine, State};
use crate::types::{
    as_xml_iter_fn, feed_fn, from_events_fn, from_xml_builder_ty, item_iter_ty, ref_ty,
    ty_from_ident,
};

/// The inner parts of the struct.
///
/// This contains all data necessary for the matching logic.
pub(crate) enum StructInner {
    /// Single-field struct declared with `#[xml(transparent)]`.
    ///
    /// Transparent struct delegate all parsing and serialising to their
    /// only field, which is why they do not need to store a lot of
    /// information and come with extra restrictions, such as:
    ///
    /// - no XML namespace can be declared (it is determined by inner type)
    /// - no XML name can be declared (it is determined by inner type)
    /// - there must be only exactly one field
    /// - that field has no `#[xml]` attribute
    Transparent {
        /// The member identifier of the only field.
        member: Member,

        /// Type of the only field.
        ty: Type,
    },

    /// A compound of fields, *not* declared as transparent.
    ///
    /// This can be a unit, tuple-like, or named struct.
    Compound {
        /// The XML namespace of the element to map the struct to.
        xml_namespace: NamespaceRef,

        /// The XML name of the element to map the struct to.
        xml_name: NameRef,

        /// The field(s) of this struct.
        inner: Compound,
    },
}

impl StructInner {
    pub(crate) fn new(meta: XmlCompoundMeta, fields: &Fields) -> Result<Self> {
        // We destructure here so that we get informed when new fields are
        // added and can handle them, either by processing them or raising
        // an error if they are present.
        let XmlCompoundMeta {
            span: meta_span,
            qname: QNameRef { namespace, name },
            exhaustive,
            debug,
            builder,
            iterator,
            on_unknown_attribute,
            on_unknown_child,
            transparent,
            discard,
            deserialize_callback,
            attribute,
            value,
        } = meta;

        // These must've been cleared by the caller. Because these being set
        // is a programming error (in xso-proc) and not a usage error, we
        // assert here instead of using reject_key!.
        assert!(builder.is_none());
        assert!(iterator.is_none());
        assert!(!debug.is_set());
        assert!(deserialize_callback.is_none());

        reject_key!(exhaustive flag not on "structs" only on "enums");
        reject_key!(attribute not on "structs" only on "enums");
        reject_key!(value not on "structs" only on "attribute-switched enum variants");

        if let Flag::Present(_) = transparent {
            reject_key!(namespace not on "transparent structs");
            reject_key!(name not on "transparent structs");
            reject_key!(on_unknown_attribute not on "transparent structs");
            reject_key!(on_unknown_child not on "transparent structs");
            reject_key!(discard vec not on "transparent structs");

            let fields_span = fields.span();
            let fields = match fields {
                Fields::Unit => {
                    return Err(Error::new(
                        fields_span,
                        "transparent structs or enum variants must have exactly one field",
                    ))
                }
                Fields::Named(FieldsNamed {
                    named: ref fields, ..
                })
                | Fields::Unnamed(FieldsUnnamed {
                    unnamed: ref fields,
                    ..
                }) => fields,
            };

            if fields.len() != 1 {
                return Err(Error::new(
                    fields_span,
                    "transparent structs or enum variants must have exactly one field",
                ));
            }

            let field = &fields[0];
            for attr in field.attrs.iter() {
                if attr.meta.path().is_ident("xml") {
                    return Err(Error::new_spanned(
                        attr,
                        "#[xml(..)] attributes are not allowed inside transparent structs",
                    ));
                }
            }
            let member = match field.ident.as_ref() {
                Some(v) => Member::Named(v.clone()),
                None => Member::Unnamed(Index {
                    span: field.ty.span(),
                    index: 0,
                }),
            };
            let ty = field.ty.clone();
            Ok(Self::Transparent { ty, member })
        } else {
            let Some(xml_namespace) = namespace else {
                return Err(Error::new(
                    meta_span,
                    "`namespace` is required on non-transparent structs",
                ));
            };

            let Some(xml_name) = name else {
                return Err(Error::new(
                    meta_span,
                    "`name` is required on non-transparent structs",
                ));
            };

            Ok(Self::Compound {
                inner: Compound::from_fields(
                    fields,
                    &xml_namespace,
                    on_unknown_attribute,
                    on_unknown_child,
                    discard,
                )?,
                xml_namespace,
                xml_name,
            })
        }
    }

    pub(crate) fn xml_name_matcher(&self) -> Result<XmlNameMatcher> {
        match self {
            Self::Transparent { ty, .. } => Ok(XmlNameMatcher::Custom(quote! {
                <#ty as ::xso::FromXml>::xml_name_matcher()
            })),
            Self::Compound {
                xml_namespace,
                xml_name,
                ..
            } => Ok(XmlNameMatcher::Specific(
                xml_namespace.to_token_stream(),
                xml_name.to_token_stream(),
            )),
        }
    }

    pub(crate) fn make_from_events_statemachine(
        &self,
        state_ty_ident: &Ident,
        output_name: &ParentRef,
        state_prefix: &str,
    ) -> Result<FromEventsSubmachine> {
        match self {
            Self::Transparent { ty, member } => {
                let from_xml_builder_ty = from_xml_builder_ty(ty.clone());
                let from_events_fn = from_events_fn(ty.clone());
                let feed_fn = feed_fn(from_xml_builder_ty.clone());

                let output_cons = match output_name {
                    ParentRef::Named(ref path) => quote! {
                        #path { #member: result }
                    },
                    ParentRef::Unnamed { .. } => quote! {
                        ( result, )
                    },
                };

                let state_name = quote::format_ident!("{}Default", state_prefix);
                let builder_data_ident = quote::format_ident!("__xso_data");

                // Here, we generate a partial statemachine which really only
                // proxies the FromXmlBuilder implementation of the inner
                // type.
                Ok(FromEventsSubmachine {
                    defs: TokenStream::default(),
                    states: vec![
                        State::new_with_builder(
                            state_name.clone(),
                            &builder_data_ident,
                            &from_xml_builder_ty,
                        )
                            .with_impl(quote! {
                                match #feed_fn(&mut #builder_data_ident, ev, ctx)? {
                                    ::core::option::Option::Some(result) => {
                                        ::core::result::Result::Ok(::core::ops::ControlFlow::Continue(#output_cons))
                                    }
                                    ::core::option::Option::None => {
                                        ::core::result::Result::Ok(::core::ops::ControlFlow::Break(Self::#state_name {
                                            #builder_data_ident,
                                        }))
                                    }
                                }
                            })
                    ],
                    init: quote! {
                        #from_events_fn(name, attrs, ctx).map(|#builder_data_ident| Self::#state_name { #builder_data_ident })
                    },
                })
            }

            Self::Compound {
                ref inner,
                ref xml_namespace,
                ref xml_name,
            } => Ok(inner
                .make_from_events_statemachine(state_ty_ident, output_name, state_prefix)?
                .with_augmented_init(|init| {
                    quote! {
                        if name.0 != #xml_namespace || name.1 != #xml_name {
                            ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch {
                                name,
                                attrs,
                            })
                        } else {
                            #init
                        }
                    }
                })),
        }
    }

    pub(crate) fn make_as_item_iter_statemachine(
        &self,
        input_name: &ParentRef,
        state_ty_ident: &Ident,
        state_prefix: &str,
        item_iter_ty_lifetime: &Lifetime,
    ) -> Result<AsItemsSubmachine> {
        match self {
            Self::Transparent { ty, member } => {
                let item_iter_ty = item_iter_ty(ty.clone(), item_iter_ty_lifetime.clone());
                let as_xml_iter_fn = as_xml_iter_fn(ty.clone());

                let state_name = quote::format_ident!("{}Default", state_prefix);
                let iter_ident = quote::format_ident!("__xso_data");

                let destructure = match input_name {
                    ParentRef::Named(ref path) => quote! {
                        #path { #member: #iter_ident }
                    },
                    ParentRef::Unnamed { .. } => quote! {
                        (#iter_ident, )
                    },
                };

                // Here, we generate a partial statemachine which really only
                // proxies the AsXml iterator implementation from the inner
                // type.
                Ok(AsItemsSubmachine {
                    defs: TokenStream::default(),
                    states: vec![State::new_with_builder(
                        state_name.clone(),
                        &iter_ident,
                        &item_iter_ty,
                    )
                    .with_mut(&iter_ident)
                    .with_impl(quote! {
                        #iter_ident.next().transpose()?
                    })],
                    destructure,
                    init: quote! {
                        #as_xml_iter_fn(#iter_ident).map(|#iter_ident| Self::#state_name { #iter_ident })?
                    },
                })
            }

            Self::Compound {
                ref inner,
                ref xml_namespace,
                ref xml_name,
            } => Ok(inner
                .make_as_item_iter_statemachine(
                    input_name,
                    state_ty_ident,
                    state_prefix,
                    item_iter_ty_lifetime,
                )?
                .with_augmented_init(|init| {
                    quote! {
                        let name = (
                            ::xso::exports::rxml::Namespace::from(#xml_namespace),
                            ::xso::exports::alloc::borrow::Cow::Borrowed(#xml_name),
                        );
                        #init
                    }
                })),
        }
    }
}

/// Definition of a struct and how to parse it.
pub(crate) struct StructDef {
    /// Name of the target type.
    target_ty_ident: Ident,

    /// Name of the builder type.
    builder_ty_ident: Ident,

    /// Name of the iterator type.
    item_iter_ty_ident: Ident,

    /// Flag whether debug mode is enabled.
    debug: bool,

    /// The matching logic and contents of the struct.
    inner: StructInner,

    /// Optional validator function to call.
    deserialize_callback: Option<Path>,
}

impl StructDef {
    /// Create a new struct from its name, meta, and fields.
    pub(crate) fn new(ident: &Ident, mut meta: XmlCompoundMeta, fields: &Fields) -> Result<Self> {
        let builder_ty_ident = match meta.builder.take() {
            Some(v) => v,
            None => quote::format_ident!("{}FromXmlBuilder", ident.to_string()),
        };

        let item_iter_ty_ident = match meta.iterator.take() {
            Some(v) => v,
            None => quote::format_ident!("{}AsXmlIterator", ident.to_string()),
        };

        let debug = meta.debug.take();
        let deserialize_callback = meta.deserialize_callback.take();

        let inner = StructInner::new(meta, fields)?;

        Ok(Self {
            inner,
            target_ty_ident: ident.clone(),
            builder_ty_ident,
            item_iter_ty_ident,
            debug: debug.is_set(),
            deserialize_callback,
        })
    }
}

impl ItemDef for StructDef {
    fn make_from_events_builder(
        &self,
        vis: &Visibility,
        name_ident: &Ident,
        attrs_ident: &Ident,
    ) -> Result<FromXmlParts> {
        let target_ty_ident = &self.target_ty_ident;
        let builder_ty_ident = &self.builder_ty_ident;
        let state_ty_ident = quote::format_ident!("{}State", builder_ty_ident);

        let defs = self
            .inner
            .make_from_events_statemachine(
                &state_ty_ident,
                &Path::from(target_ty_ident.clone()).into(),
                "Struct",
            )?
            .compile()
            .render(
                vis,
                builder_ty_ident,
                &state_ty_ident,
                &TypePath {
                    qself: None,
                    path: target_ty_ident.clone().into(),
                }
                .into(),
                self.deserialize_callback.as_ref(),
            )?;

        Ok(FromXmlParts {
            defs,
            from_events_body: quote! {
                #builder_ty_ident::new(#name_ident, #attrs_ident, ctx)
            },
            builder_ty_ident: builder_ty_ident.clone(),
            name_matcher: self.inner.xml_name_matcher()?,
        })
    }

    fn make_as_xml_iter(&self, vis: &Visibility) -> Result<AsXmlParts> {
        let target_ty_ident = &self.target_ty_ident;
        let item_iter_ty_ident = &self.item_iter_ty_ident;
        let item_iter_ty_lifetime = Lifetime {
            apostrophe: Span::call_site(),
            ident: Ident::new("xso_proc_as_xml_iter_lifetime", Span::call_site()),
        };
        let item_iter_ty = Type::Path(TypePath {
            qself: None,
            path: Path {
                leading_colon: None,
                segments: [PathSegment {
                    ident: item_iter_ty_ident.clone(),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: token::Lt {
                            spans: [Span::call_site()],
                        },
                        args: [GenericArgument::Lifetime(item_iter_ty_lifetime.clone())]
                            .into_iter()
                            .collect(),
                        gt_token: token::Gt {
                            spans: [Span::call_site()],
                        },
                    }),
                }]
                .into_iter()
                .collect(),
            },
        });
        let state_ty_ident = quote::format_ident!("{}State", item_iter_ty_ident);

        let defs = self
            .inner
            .make_as_item_iter_statemachine(
                &Path::from(target_ty_ident.clone()).into(),
                &state_ty_ident,
                "Struct",
                &item_iter_ty_lifetime,
            )?
            .compile()
            .render(
                vis,
                &ref_ty(
                    ty_from_ident(target_ty_ident.clone()).into(),
                    item_iter_ty_lifetime.clone(),
                ),
                &state_ty_ident,
                &item_iter_ty_lifetime,
                &item_iter_ty,
            )?;

        Ok(AsXmlParts {
            defs,
            as_xml_iter_body: quote! {
                #item_iter_ty_ident::new(self)
            },
            item_iter_ty,
            item_iter_ty_lifetime,
        })
    }

    fn debug(&self) -> bool {
        self.debug
    }
}
