// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Compound (struct or enum variant) field types

use proc_macro2::{Span, TokenStream};
use syn::{spanned::Spanned, *};

use rxml_validation::NcName;

use crate::compound::Compound;
use crate::error_message::ParentRef;
use crate::meta::{AmountConstraint, Flag, NameRef, NamespaceRef, QNameRef, XmlFieldMeta};
use crate::scope::{AsItemsScope, FromEventsScope};

mod attribute;
mod child;
#[cfg(feature = "minidom")]
mod element;
mod flag;
mod text;

use self::attribute::AttributeField;
use self::child::{ChildField, ExtractDef};
#[cfg(feature = "minidom")]
use self::element::ElementField;
use self::flag::FlagField;
use self::text::TextField;

/// Code slices necessary for declaring and initializing a temporary variable
/// for parsing purposes.
pub(crate) struct FieldTempInit {
    /// The type of the temporary variable.
    pub(crate) ty: Type,

    /// The initializer for the temporary variable.
    pub(crate) init: TokenStream,
}

/// Configure how a nested field builder selects child elements.
pub(crate) enum NestedMatcher {
    /// Matches a specific child element fallabily.
    Selective(
        /// Expression which evaluates to `Result<T, FromEventsError>`,
        /// consuming `name: rxml::QName` and `attrs: rxml::AttrMap`.
        ///
        /// If the `name` and `attrs` allow starting to parse the child
        /// element as a value of this field, `Ok(_)` must be returned. If
        /// the `name` and `attrs` are those of an element which *could*
        /// be a value of this field, but they have invalid contents,
        /// `Err(FromEventsError::Invalid(_))` must be returned. Depending
        /// on the field kind, it may also be acceptable to return the
        /// `Invalid` variant if the data is valid, but no further child
        /// element can be accepted into the value.
        ///
        /// Otherwise, the `name` and `attrs` must be returned *unchanged* in
        /// a `FromEventsError::Mismatch { .. }` variant. In that case, the
        /// implementation in `Compound` will let the next field attempt to
        /// parse the child element.
        ///
        /// `T` must be the type specified in the
        /// [`FieldBuilderPart::Nested::builder`]  field.
        TokenStream,
    ),

    #[cfg_attr(not(feature = "minidom"), allow(dead_code))]
    /// Matches any child element not matched by another matcher.
    ///
    /// Only a single field may use this variant, otherwise an error is
    /// raised during execution of the proc macro.
    Fallback(
        /// Expression which evaluates to `T` (or `return`s an error),
        /// consuming `name: rxml::QName` and `attrs: rxml::AttrMap`.
        ///
        /// Unlike the [`Selective`][`Self::Selective`] variant, this
        /// expression must always evaluate to an instance of `T`. If that is
        /// not possible, the expression must diverge, most commonly using
        /// `return` with a `Err::<_, xso::error::Error>(_)`.
        ///
        /// `T` must be the type specified in the
        /// [`FieldBuilderPart::Nested::builder`]  field.
        TokenStream,
    ),
}

/// Describe how a struct or enum variant's member is parsed from XML data.
///
/// This struct is returned from [`FieldDef::make_builder_part`] and
/// contains code snippets and instructions for
/// [`Compound::make_from_events_statemachine`][`crate::compound::Compound::make_from_events_statemachine`]
/// to parse the field's data from XML.
pub(crate) enum FieldBuilderPart {
    /// Parse a field from the item's element's start event.
    Init {
        /// Expression and type which extracts the field's data from the
        /// element's start event.
        value: FieldTempInit,
    },

    /// Parse a field from text events.
    Text {
        /// Expression and type which initializes a buffer to use during
        /// parsing.
        value: FieldTempInit,

        /// Statement which takes text and accumulates it into the temporary
        /// value declared via `value`.
        collect: TokenStream,

        /// Expression which evaluates to the field's type, consuming the
        /// temporary value.
        finalize: TokenStream,
    },

    /// Parse a field from child element events.
    Nested {
        /// Additional definition items which need to be inserted at module
        /// level for the rest of the implementation to work.
        extra_defs: TokenStream,

        /// Expression and type which initializes a buffer to use during
        /// parsing.
        value: FieldTempInit,

        /// Configure child matching behaviour for this field. See
        /// [`NestedMatcher`] for options.
        matcher: NestedMatcher,

        /// Type implementing `xso::FromEventsBuilder` which parses the child
        /// element.
        ///
        /// This type is returned by the expressions in
        /// [`matcher`][`Self::Nested::matcher`].
        builder: Type,

        /// Expression which consumes the value stored in the identifier
        /// [`crate::common::FromEventsScope::substate_result`][`FromEventsScope::substate_result`]
        /// and somehow collects it into the field declared with
        /// [`value`][`Self::Nested::value`].
        collect: TokenStream,

        /// Expression which consumes the data from the field declared with
        /// [`value`][`Self::Nested::value`] and converts it into the field's
        /// type.
        finalize: TokenStream,
    },
}

/// Describe how a struct or enum variant's member is converted to XML data.
///
/// This struct is returned from [`FieldDef::make_iterator_part`] and
/// contains code snippets and instructions for
/// [`Compound::make_into_events_statemachine`][`crate::compound::Compound::make_into_events_statemachine`]
/// to convert the field's data into XML.
pub(crate) enum FieldIteratorPart {
    /// The field is emitted as part of StartElement.
    Header {
        /// An expression which consumes the field's value and returns a
        /// `Item`.
        generator: TokenStream,
    },

    /// The field is emitted as text item.
    Text {
        /// An expression which consumes the field's value and returns a
        /// String, which is then emitted as text data.
        generator: TokenStream,
    },

    /// The field is emitted as series of items which form a child element.
    Content {
        /// Additional definition items which need to be inserted at module
        /// level for the rest of the implementation to work.
        extra_defs: TokenStream,

        /// Expression and type which initializes the nested iterator.
        ///
        /// Note that this is evaluated at construction time of the iterator.
        /// Fields of this variant do not get access to their original data,
        /// unless they carry it in the contents of this `value`.
        value: FieldTempInit,

        /// An expression which uses the value (mutably) and evaluates to
        /// a Result<Option<Item>, Error>. Once the state returns None, the
        /// processing will advance to the next state.
        generator: TokenStream,
    },
}

trait Field {
    /// Construct the builder pieces for this field.
    ///
    /// `container_name` must be a reference to the compound's type, so that
    /// it can be used for error messages.
    ///
    /// `member` and `ty` refer to the field itself.
    fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldBuilderPart>;

    /// Construct the iterator pieces for this field.
    ///
    /// `bound_name` must be the name to which the field's value is bound in
    /// the iterator code.
    ///
    /// `member` and `ty` refer to the field itself.
    ///
    /// `bound_name` is the name under which the field's value is accessible
    /// in the various parts of the code.
    fn make_iterator_part(
        &self,
        scope: &AsItemsScope,
        container_name: &ParentRef,
        bound_name: &Ident,
        member: &Member,
        ty: &Type,
    ) -> Result<FieldIteratorPart>;

    /// Return true if and only if this field captures text content.
    fn captures_text(&self) -> bool {
        false
    }
}

fn default_name(span: Span, name: Option<NameRef>, field_ident: Option<&Ident>) -> Result<NameRef> {
    match name {
        Some(v) => Ok(v),
        None => match field_ident {
            None => Err(Error::new(
                span,
                "name must be explicitly specified with the `name` key on unnamed fields",
            )),
            Some(field_ident) => match NcName::try_from(field_ident.to_string()) {
                Ok(value) => Ok(NameRef::Literal {
                    span: field_ident.span(),
                    value,
                }),
                Err(e) => Err(Error::new(
                    field_ident.span(),
                    format!("invalid XML name: {}", e),
                )),
            },
        },
    }
}

/// Construct a new field implementation from the meta attributes.
///
/// `field_ident` is, for some field types, used to infer an XML name if
/// it is not specified explicitly.
///
/// `field_ty` is needed for type inference on extracted fields.
///
/// `container_namespace` is used in some cases to insert a default
/// namespace.
fn new_field(
    meta: XmlFieldMeta,
    field_ident: Option<&Ident>,
    field_ty: &Type,
    container_namespace: &NamespaceRef,
) -> Result<Box<dyn Field>> {
    match meta {
        XmlFieldMeta::Attribute {
            span,
            qname: QNameRef { namespace, name },
            default_,
            type_,
            codec,
        } => {
            let xml_name = default_name(span, name, field_ident)?;

            // This would've been taken via `XmlFieldMeta::take_type` if
            // this field was within an extract where a `type_` is legal
            // to have.
            if let Some(type_) = type_ {
                return Err(Error::new_spanned(
                    type_,
                    "specifying `type_` on fields inside structs and enum variants is redundant and not allowed."
                ));
            }

            Ok(Box::new(AttributeField {
                xml_name,
                xml_namespace: namespace,
                default_,
                codec,
            }))
        }

        XmlFieldMeta::Text {
            span: _,
            codec,
            type_,
        } => {
            // This would've been taken via `XmlFieldMeta::take_type` if
            // this field was within an extract where a `type_` is legal
            // to have.
            if let Some(type_) = type_ {
                return Err(Error::new_spanned(
                    type_,
                    "specifying `type_` on fields inside structs and enum variants is redundant and not allowed."
                ));
            }

            Ok(Box::new(TextField { codec }))
        }

        XmlFieldMeta::Child {
            span: _,
            default_,
            amount,
        } => {
            if let Some(AmountConstraint::Any(ref amount_span)) = amount {
                if let Flag::Present(ref flag_span) = default_ {
                    let mut err =
                        Error::new(*flag_span, "`default` has no meaning for child collections");
                    err.combine(Error::new(
                        *amount_span,
                        "the field is treated as a collection because of this `n` value",
                    ));
                    return Err(err);
                }
            }

            Ok(Box::new(ChildField {
                default_,
                amount: amount.unwrap_or(AmountConstraint::FixedSingle(Span::call_site())),
                extract: None,
            }))
        }

        XmlFieldMeta::Extract {
            span,
            default_,
            qname: QNameRef { namespace, name },
            amount,
            fields,
            on_unknown_attribute,
            on_unknown_child,
        } => {
            let xml_namespace = namespace.unwrap_or_else(|| container_namespace.clone());
            let xml_name = default_name(span, name, field_ident)?;

            let amount = amount.unwrap_or(AmountConstraint::FixedSingle(Span::call_site()));
            match amount {
                AmountConstraint::Any(ref amount) => {
                    if let Flag::Present(default_) = default_ {
                        let mut err = Error::new(
                            default_,
                            "default cannot be set when collecting into a collection",
                        );
                        err.combine(Error::new(
                            *amount,
                            "`n` was set to a non-1 value here, which enables collection logic",
                        ));
                        return Err(err);
                    }
                }
                AmountConstraint::FixedSingle(_) => (),
            }

            let mut field_defs = Vec::new();
            let allow_inference =
                matches!(amount, AmountConstraint::FixedSingle(_)) && fields.len() == 1;
            for (i, mut field) in fields.into_iter().enumerate() {
                let field_ty = match field.take_type() {
                    Some(v) => v,
                    None => {
                        if allow_inference {
                            field_ty.clone()
                        } else {
                            return Err(Error::new(
                            field.span(),
                            "extracted field must specify a type explicitly when extracting into a collection or when extracting more than one field."
                        ));
                        }
                    }
                };

                field_defs.push(FieldDef::from_extract(
                    field,
                    i as u32,
                    &field_ty,
                    &xml_namespace,
                ));
            }
            let parts = Compound::from_field_defs(
                field_defs,
                on_unknown_attribute,
                on_unknown_child,
                vec![],
            )?;

            Ok(Box::new(ChildField {
                default_,
                amount,
                extract: Some(ExtractDef {
                    xml_namespace,
                    xml_name,
                    parts,
                }),
            }))
        }

        #[cfg(feature = "minidom")]
        XmlFieldMeta::Element {
            span,
            default_,
            amount,
        } => Ok(Box::new(ElementField {
            default_,
            amount: amount.unwrap_or(AmountConstraint::FixedSingle(span)),
        })),

        #[cfg(not(feature = "minidom"))]
        XmlFieldMeta::Element {
            span,
            amount,
            default_,
        } => {
            let _ = amount;
            let _ = default_;
            Err(Error::new(
                span,
                "#[xml(element)] requires xso to be built with the \"minidom\" feature.",
            ))
        }

        XmlFieldMeta::Flag {
            span,
            qname: QNameRef { namespace, name },
        } => {
            let xml_namespace = namespace.unwrap_or_else(|| container_namespace.clone());
            let xml_name = default_name(span, name, field_ident)?;
            Ok(Box::new(FlagField {
                xml_namespace,
                xml_name,
            }))
        }
    }
}

/// Definition of a single field in a compound.
///
/// See [`Compound`][`crate::compound::Compound`] for more information on
/// compounds in general.
pub(crate) struct FieldDef {
    /// A span which refers to the field's definition.
    span: Span,

    /// The member identifying the field.
    member: Member,

    /// The type of the field.
    ty: Type,

    /// The way the field is mapped to XML.
    inner: Box<dyn Field>,
}

impl FieldDef {
    /// Create a new field definition from its declaration.
    ///
    /// The `index` must be the zero-based index of the field even for named
    /// fields.
    pub(crate) fn from_field(
        field: &syn::Field,
        index: u32,
        container_namespace: &NamespaceRef,
    ) -> Result<Self> {
        let (member, ident) = match field.ident.as_ref() {
            Some(v) => (Member::Named(v.clone()), Some(v)),
            None => (
                Member::Unnamed(Index {
                    index,
                    // We use the type's span here, because `field.span()`
                    // will visually point at the `#[xml(..)]` meta, which is
                    // not helpful when glancing at error messages referring
                    // to the field itself.
                    span: field.ty.span(),
                }),
                None,
            ),
        };
        // This will either be the field's identifier's span (for named
        // fields) or the field's type (for unnamed fields), which should give
        // the user a good visual feedback about which field an error message
        // is.
        let field_span = member.span();
        let meta = XmlFieldMeta::parse_from_attributes(&field.attrs, &field_span)?;
        let ty = field.ty.clone();

        Ok(Self {
            span: field_span,
            inner: new_field(meta, ident, &ty, container_namespace)?,
            member,
            ty,
        })
    }

    /// Create a new field definition from its declaration.
    ///
    /// The `index` must be the zero-based index of the field even for named
    /// fields.
    pub(crate) fn from_extract(
        meta: XmlFieldMeta,
        index: u32,
        ty: &Type,
        container_namespace: &NamespaceRef,
    ) -> Result<Self> {
        let span = meta.span();
        Ok(Self {
            span,
            member: Member::Unnamed(Index { index, span }),
            ty: ty.clone(),
            inner: new_field(meta, None, ty, container_namespace)?,
        })
    }

    /// Access the [`syn::Member`] identifying this field in the original
    /// type.
    pub(crate) fn member(&self) -> &Member {
        &self.member
    }

    /// Access the field's type.
    pub(crate) fn ty(&self) -> &Type {
        &self.ty
    }

    /// Construct the builder pieces for this field.
    ///
    /// `container_name` must be a reference to the compound's type, so that
    /// it can be used for error messages.
    pub(crate) fn make_builder_part(
        &self,
        scope: &FromEventsScope,
        container_name: &ParentRef,
    ) -> Result<FieldBuilderPart> {
        self.inner
            .make_builder_part(scope, container_name, &self.member, &self.ty)
    }

    /// Construct the iterator pieces for this field.
    ///
    /// `bound_name` must be the name to which the field's value is bound in
    /// the iterator code.
    pub(crate) fn make_iterator_part(
        &self,
        scope: &AsItemsScope,
        container_name: &ParentRef,
        bound_name: &Ident,
    ) -> Result<FieldIteratorPart> {
        self.inner
            .make_iterator_part(scope, container_name, bound_name, &self.member, &self.ty)
    }

    /// Return true if this field's parsing consumes text data.
    pub(crate) fn is_text_field(&self) -> bool {
        self.inner.captures_text()
    }

    /// Return a span which points at the field's definition.'
    pub(crate) fn span(&self) -> Span {
        self.span
    }
}
