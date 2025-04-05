// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Handling of enums

use std::collections::HashMap;

use proc_macro2::Span;
use quote::quote;
use syn::*;

use crate::common::{AsXmlParts, FromXmlParts, ItemDef};
use crate::compound::Compound;
use crate::error_message::ParentRef;
use crate::meta::{reject_key, Flag, NameRef, NamespaceRef, QNameRef, XmlCompoundMeta};
use crate::state::{AsItemsStateMachine, FromEventsStateMachine};
use crate::structs::StructInner;
use crate::types::{ref_ty, ty_from_ident};

/// The definition of an enum variant, switched on the XML element's name,
/// inside a [`NameSwitchedEnum`].
struct NameVariant {
    /// The XML name of the element to map the enum variant to.
    name: NameRef,

    /// The name of the variant
    ident: Ident,

    /// The field(s) of this struct.
    inner: Compound,
}

impl NameVariant {
    /// Construct a new name-selected variant from its declaration.
    fn new(decl: &Variant, enum_namespace: &NamespaceRef) -> Result<Self> {
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
        } = XmlCompoundMeta::parse_from_attributes(&decl.attrs)?;

        reject_key!(debug flag not on "enum variants" only on "enums and structs");
        reject_key!(exhaustive flag not on "enum variants" only on "enums");
        reject_key!(namespace not on "enum variants" only on "enums and structs");
        reject_key!(builder not on "enum variants" only on "enums and structs");
        reject_key!(iterator not on "enum variants" only on "enums and structs");
        reject_key!(transparent flag not on "named enum variants" only on "structs");
        reject_key!(deserialize_callback not on "enum variants" only on "enums and structs");

        let Some(name) = name else {
            return Err(Error::new(meta_span, "`name` is required on enum variants"));
        };

        Ok(Self {
            name,
            ident: decl.ident.clone(),
            inner: Compound::from_fields(
                &decl.fields,
                enum_namespace,
                on_unknown_attribute,
                on_unknown_child,
                discard,
            )?,
        })
    }

    fn make_from_events_statemachine(
        &self,
        enum_ident: &Ident,
        state_ty_ident: &Ident,
    ) -> Result<FromEventsStateMachine> {
        let xml_name = &self.name;

        Ok(self
            .inner
            .make_from_events_statemachine(
                state_ty_ident,
                &ParentRef::Named(Path {
                    leading_colon: None,
                    segments: [
                        PathSegment::from(enum_ident.clone()),
                        self.ident.clone().into(),
                    ]
                    .into_iter()
                    .collect(),
                }),
                &self.ident.to_string(),
            )?
            .with_augmented_init(|init| {
                quote! {
                    if name.1 != #xml_name {
                        ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch {
                            name,
                            attrs,
                        })
                    } else {
                        #init
                    }
                }
            })
            .compile())
    }

    fn make_as_item_iter_statemachine(
        &self,
        xml_namespace: &NamespaceRef,
        enum_ident: &Ident,
        state_ty_ident: &Ident,
        item_iter_ty_lifetime: &Lifetime,
    ) -> Result<AsItemsStateMachine> {
        let xml_name = &self.name;

        Ok(self
            .inner
            .make_as_item_iter_statemachine(
                &ParentRef::Named(Path {
                    leading_colon: None,
                    segments: [
                        PathSegment::from(enum_ident.clone()),
                        self.ident.clone().into(),
                    ]
                    .into_iter()
                    .collect(),
                }),
                state_ty_ident,
                &self.ident.to_string(),
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
            })
            .compile())
    }
}

/// The definition of a enum which switches based on the XML element name,
/// with the XML namespace fixed.
struct NameSwitchedEnum {
    /// The XML namespace of the element to map the enum to.
    namespace: NamespaceRef,

    /// The variants of the enum.
    variants: Vec<NameVariant>,

    /// Flag indicating whether the enum is exhaustive.
    exhaustive: bool,
}

impl NameSwitchedEnum {
    fn new<'x, I: IntoIterator<Item = &'x Variant>>(
        namespace: NamespaceRef,
        exhaustive: Flag,
        variant_iter: I,
    ) -> Result<Self> {
        let mut variants = Vec::new();
        let mut seen_names = HashMap::new();
        for variant in variant_iter {
            let variant = NameVariant::new(variant, &namespace)?;
            if let Some(other) = seen_names.get(&variant.name) {
                return Err(Error::new_spanned(
                    variant.name,
                    format!(
                        "duplicate `name` in enum: variants {} and {} have the same XML name",
                        other, variant.ident
                    ),
                ));
            }
            seen_names.insert(variant.name.clone(), variant.ident.clone());
            variants.push(variant);
        }

        Ok(Self {
            namespace,
            variants,
            exhaustive: exhaustive.is_set(),
        })
    }

    /// Build the deserialisation statemachine for the name-switched enum.
    fn make_from_events_statemachine(
        &self,
        target_ty_ident: &Ident,
        state_ty_ident: &Ident,
    ) -> Result<FromEventsStateMachine> {
        let xml_namespace = &self.namespace;

        let mut statemachine = FromEventsStateMachine::new();
        for variant in self.variants.iter() {
            statemachine
                .merge(variant.make_from_events_statemachine(target_ty_ident, state_ty_ident)?);
        }

        statemachine.set_pre_init(quote! {
            if name.0 != #xml_namespace {
                return ::core::result::Result::Err(::xso::error::FromEventsError::Mismatch {
                    name,
                    attrs,
                })
            }
        });

        if self.exhaustive {
            let mismatch_err = format!("This is not a {} element.", target_ty_ident);
            statemachine.set_fallback(quote! {
                ::core::result::Result::Err(::xso::error::FromEventsError::Invalid(
                    ::xso::error::Error::Other(#mismatch_err),
                ))
            })
        }

        Ok(statemachine)
    }

    /// Build the serialisation statemachine for the name-switched enum.
    fn make_as_item_iter_statemachine(
        &self,
        target_ty_ident: &Ident,
        state_ty_ident: &Ident,
        item_iter_ty_lifetime: &Lifetime,
    ) -> Result<AsItemsStateMachine> {
        let mut statemachine = AsItemsStateMachine::new();
        for variant in self.variants.iter() {
            statemachine.merge(variant.make_as_item_iter_statemachine(
                &self.namespace,
                target_ty_ident,
                state_ty_ident,
                item_iter_ty_lifetime,
            )?);
        }

        Ok(statemachine)
    }
}

/// The definition of an enum variant in a [`DynamicEnum`].
struct DynamicVariant {
    /// The identifier of the enum variant.
    ident: Ident,

    /// The definition of the struct-like which resembles the enum variant.
    inner: StructInner,
}

impl DynamicVariant {
    fn new(variant: &Variant) -> Result<Self> {
        let ident = variant.ident.clone();
        let meta = XmlCompoundMeta::parse_from_attributes(&variant.attrs)?;

        // We destructure here so that we get informed when new fields are
        // added and can handle them, either by processing them or raising
        // an error if they are present.
        let XmlCompoundMeta {
            span: _,
            qname: _, // used by StructInner
            ref exhaustive,
            ref debug,
            ref builder,
            ref iterator,
            on_unknown_attribute: _, // used by StructInner
            on_unknown_child: _,     // used by StructInner
            transparent: _,          // used by StructInner
            discard: _,              // used by StructInner
            ref deserialize_callback,
        } = meta;

        reject_key!(debug flag not on "enum variants" only on "enums and structs");
        reject_key!(exhaustive flag not on "enum variants" only on "enums");
        reject_key!(builder not on "enum variants" only on "enums and structs");
        reject_key!(iterator not on "enum variants" only on "enums and structs");
        reject_key!(deserialize_callback not on "enum variants" only on "enums and structs");

        let inner = StructInner::new(meta, &variant.fields)?;
        Ok(Self { ident, inner })
    }
}

/// The definition of an enum where each variant is a completely unrelated
/// possible XML subtree.
struct DynamicEnum {
    /// The enum variants.
    variants: Vec<DynamicVariant>,
}

impl DynamicEnum {
    fn new<'x, I: IntoIterator<Item = &'x Variant>>(variant_iter: I) -> Result<Self> {
        let mut variants = Vec::new();
        for variant in variant_iter {
            variants.push(DynamicVariant::new(variant)?);
        }

        Ok(Self { variants })
    }

    /// Build the deserialisation statemachine for the dynamic enum.
    fn make_from_events_statemachine(
        &self,
        target_ty_ident: &Ident,
        state_ty_ident: &Ident,
    ) -> Result<FromEventsStateMachine> {
        let mut statemachine = FromEventsStateMachine::new();
        for variant in self.variants.iter() {
            let submachine = variant.inner.make_from_events_statemachine(
                state_ty_ident,
                &ParentRef::Named(Path {
                    leading_colon: None,
                    segments: [
                        PathSegment::from(target_ty_ident.clone()),
                        variant.ident.clone().into(),
                    ]
                    .into_iter()
                    .collect(),
                }),
                &variant.ident.to_string(),
            )?;

            statemachine.merge(submachine.compile());
        }

        Ok(statemachine)
    }

    /// Build the serialisation statemachine for the dynamic enum.
    fn make_as_item_iter_statemachine(
        &self,
        target_ty_ident: &Ident,
        state_ty_ident: &Ident,
        item_iter_ty_lifetime: &Lifetime,
    ) -> Result<AsItemsStateMachine> {
        let mut statemachine = AsItemsStateMachine::new();
        for variant in self.variants.iter() {
            let submachine = variant.inner.make_as_item_iter_statemachine(
                &ParentRef::Named(Path {
                    leading_colon: None,
                    segments: [
                        PathSegment::from(target_ty_ident.clone()),
                        variant.ident.clone().into(),
                    ]
                    .into_iter()
                    .collect(),
                }),
                state_ty_ident,
                &variant.ident.to_string(),
                item_iter_ty_lifetime,
            )?;

            statemachine.merge(submachine.compile());
        }

        Ok(statemachine)
    }
}

/// The definition of an enum.
enum EnumInner {
    /// The enum switches based on the XML name of the element, with the XML
    /// namespace fixed.
    NameSwitched(NameSwitchedEnum),

    /// The enum consists of variants with entirely unrelated XML structures.
    Dynamic(DynamicEnum),
}

impl EnumInner {
    fn new<'x, I: IntoIterator<Item = &'x Variant>>(
        meta: XmlCompoundMeta,
        variant_iter: I,
    ) -> Result<Self> {
        // We destructure here so that we get informed when new fields are
        // added and can handle them, either by processing them or raising
        // an error if they are present.
        let XmlCompoundMeta {
            span: _,
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
        } = meta;

        // These must've been cleared by the caller. Because these being set
        // is a programming error (in xso-proc) and not a usage error, we
        // assert here instead of using reject_key!.
        assert!(builder.is_none());
        assert!(iterator.is_none());
        assert!(!debug.is_set());
        assert!(deserialize_callback.is_none());

        reject_key!(name not on "enums" only on "their variants");
        reject_key!(transparent flag not on "enums" only on "structs");
        reject_key!(on_unknown_attribute not on "enums" only on "enum variants and structs");
        reject_key!(on_unknown_child not on "enums" only on "enum variants and structs");
        reject_key!(discard vec not on "enums" only on "enum variants and structs");

        if let Some(namespace) = namespace {
            Ok(Self::NameSwitched(NameSwitchedEnum::new(
                namespace,
                exhaustive,
                variant_iter,
            )?))
        } else {
            reject_key!(exhaustive flag not on "dynamic enums" only on "name-switched enums");
            Ok(Self::Dynamic(DynamicEnum::new(variant_iter)?))
        }
    }

    /// Build the deserialisation statemachine for the enum.
    fn make_from_events_statemachine(
        &self,
        target_ty_ident: &Ident,
        state_ty_ident: &Ident,
    ) -> Result<FromEventsStateMachine> {
        match self {
            Self::NameSwitched(ref inner) => {
                inner.make_from_events_statemachine(target_ty_ident, state_ty_ident)
            }
            Self::Dynamic(ref inner) => {
                inner.make_from_events_statemachine(target_ty_ident, state_ty_ident)
            }
        }
    }

    /// Build the serialisation statemachine for the enum.
    fn make_as_item_iter_statemachine(
        &self,
        target_ty_ident: &Ident,
        state_ty_ident: &Ident,
        item_iter_ty_lifetime: &Lifetime,
    ) -> Result<AsItemsStateMachine> {
        match self {
            Self::NameSwitched(ref inner) => inner.make_as_item_iter_statemachine(
                target_ty_ident,
                state_ty_ident,
                item_iter_ty_lifetime,
            ),
            Self::Dynamic(ref inner) => inner.make_as_item_iter_statemachine(
                target_ty_ident,
                state_ty_ident,
                item_iter_ty_lifetime,
            ),
        }
    }
}

/// Definition of an enum and how to parse it.
pub(crate) struct EnumDef {
    /// Implementation of the enum itself
    inner: EnumInner,

    /// Name of the target type.
    target_ty_ident: Ident,

    /// Name of the builder type.
    builder_ty_ident: Ident,

    /// Name of the iterator type.
    item_iter_ty_ident: Ident,

    /// Flag whether debug mode is enabled.
    debug: bool,

    /// Optional validator function to call.
    deserialize_callback: Option<Path>,
}

impl EnumDef {
    /// Create a new enum from its name, meta, and variants.
    pub(crate) fn new<'x, I: IntoIterator<Item = &'x Variant>>(
        ident: &Ident,
        mut meta: XmlCompoundMeta,
        variant_iter: I,
    ) -> Result<Self> {
        let builder_ty_ident = match meta.builder.take() {
            Some(v) => v,
            None => quote::format_ident!("{}FromXmlBuilder", ident.to_string()),
        };

        let item_iter_ty_ident = match meta.iterator.take() {
            Some(v) => v,
            None => quote::format_ident!("{}AsXmlIterator", ident.to_string()),
        };

        let debug = meta.debug.take().is_set();
        let deserialize_callback = meta.deserialize_callback.take();

        Ok(Self {
            inner: EnumInner::new(meta, variant_iter)?,
            target_ty_ident: ident.clone(),
            builder_ty_ident,
            item_iter_ty_ident,
            debug,
            deserialize_callback,
        })
    }
}

impl ItemDef for EnumDef {
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
            .make_from_events_statemachine(target_ty_ident, &state_ty_ident)?
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
                #builder_ty_ident::new(#name_ident, #attrs_ident)
            },
            builder_ty_ident: builder_ty_ident.clone(),
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
                target_ty_ident,
                &state_ty_ident,
                &item_iter_ty_lifetime,
            )?
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
