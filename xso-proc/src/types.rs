// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Module with specific [`syn::Type`] constructors.

use proc_macro2::Span;
use syn::{spanned::Spanned, *};

/// Construct a [`syn::Type`] referring to `::xso::exports::rxml::Namespace`.
pub(crate) fn namespace_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("rxml", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Namespace", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to `::xso::exports::rxml::NcNameStr`.
pub(crate) fn ncnamestr_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("rxml", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("NcNameStr", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to `Cow<#lifetime, #ty>`.
pub(crate) fn cow_ty(ty: Type, lifetime: Lifetime) -> Type {
    let span = ty.span();
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("alloc", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("borrow", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Cow", span),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: token::Lt { spans: [span] },
                        args: [
                            GenericArgument::Lifetime(lifetime),
                            GenericArgument::Type(ty),
                        ]
                        .into_iter()
                        .collect(),
                        gt_token: token::Gt { spans: [span] },
                    }),
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to
/// `Cow<#lifetime, ::rxml::NcNameStr>`.
pub(crate) fn ncnamestr_cow_ty(ty_span: Span, lifetime: Lifetime) -> Type {
    cow_ty(ncnamestr_ty(ty_span), lifetime)
}

/// Construct a [`syn::Expr`] referring to
/// `<#ty as ::xso::FromXmlText>::from_xml_text`.
pub(crate) fn from_xml_text_fn(ty: Type) -> Expr {
    let span = ty.span();
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: Some(QSelf {
            lt_token: syn::token::Lt { spans: [span] },
            ty: Box::new(ty),
            position: 2,
            as_token: Some(syn::token::As { span }),
            gt_token: syn::token::Gt { spans: [span] },
        }),
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("FromXmlText", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("from_xml_text", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Expr`] referring to
/// `<#ty as ::xso::AsOptionalXmlText>::as_optional_xml_text`.
pub(crate) fn as_optional_xml_text_fn(ty: Type) -> Expr {
    let span = ty.span();
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: Some(QSelf {
            lt_token: syn::token::Lt { spans: [span] },
            ty: Box::new(ty),
            position: 2,
            as_token: Some(syn::token::As { span }),
            gt_token: syn::token::Gt { spans: [span] },
        }),
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("AsOptionalXmlText", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("as_optional_xml_text", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Expr`] referring to
/// `<#of_ty as ::core::default::Default>::default`.
pub(crate) fn default_fn(of_ty: Type) -> Expr {
    let span = of_ty.span();
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: Some(QSelf {
            lt_token: syn::token::Lt { spans: [span] },
            ty: Box::new(of_ty),
            position: 3,
            as_token: Some(syn::token::As { span }),
            gt_token: syn::token::Gt { spans: [span] },
        }),
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("core", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("default", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Default", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("default", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to `::alloc::string::String`.
pub(crate) fn string_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("alloc", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("string", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("String", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Expr`] referring to
/// `<#ty as ::xso::AsXmlText>::as_xml_text`.
pub(crate) fn as_xml_text_fn(ty: Type) -> Expr {
    let span = ty.span();
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: Some(QSelf {
            lt_token: syn::token::Lt { spans: [span] },
            ty: Box::new(ty),
            position: 2,
            as_token: Some(syn::token::As { span }),
            gt_token: syn::token::Gt { spans: [span] },
        }),
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("AsXmlText", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("as_xml_text", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Path`] referring to `::xso::TextCodec::<#for_ty>`,
/// returning the span of `for_ty` alongside it.
///
/// The span used is `codec_span`, in order to ensure that error messages
/// about a missing implementation point at the codec, not at the type.
fn text_codec_of(for_ty: Type, codec_span: Span) -> Path {
    let span = codec_span;
    Path {
        leading_colon: Some(syn::token::PathSep {
            spans: [span, span],
        }),
        segments: [
            PathSegment {
                ident: Ident::new("xso", span),
                arguments: PathArguments::None,
            },
            PathSegment {
                ident: Ident::new("TextCodec", span),
                arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    colon2_token: Some(syn::token::PathSep {
                        spans: [span, span],
                    }),
                    lt_token: syn::token::Lt { spans: [span] },
                    args: [GenericArgument::Type(for_ty)].into_iter().collect(),
                    gt_token: syn::token::Gt { spans: [span] },
                }),
            },
        ]
        .into_iter()
        .collect(),
    }
}

/// Construct a [`syn::Expr`] referring to
/// `::xso::TextCodec::<#for_ty>::encode`.
///
/// The span used is `codec_span`, in order to ensure that error messages
/// about a missing implementation point at the codec, not at the type.
pub(crate) fn text_codec_encode_fn(for_ty: Type, codec_span: Span) -> Expr {
    let mut path = text_codec_of(for_ty, codec_span);
    path.segments.push(PathSegment {
        ident: Ident::new("encode", codec_span),
        arguments: PathArguments::None,
    });
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: None,
        path,
    })
}

/// Construct a [`syn::Expr`] referring to
/// `::xso::TextCodec::<#for_ty>::decode`.
///
/// The span used is `codec_span`, in order to ensure that error messages
/// about a missing implementation point at the codec, not at the type.
pub(crate) fn text_codec_decode_fn(for_ty: Type, codec_span: Span) -> Expr {
    let mut path = text_codec_of(for_ty, codec_span);
    path.segments.push(PathSegment {
        ident: Ident::new("decode", codec_span),
        arguments: PathArguments::None,
    });
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: None,
        path,
    })
}

/// Construct a [`syn::Type`] for `&#lifetime #ty`.
pub(crate) fn ref_ty(ty: Type, lifetime: Lifetime) -> Type {
    let span = ty.span();
    Type::Reference(TypeReference {
        and_token: token::And { spans: [span] },
        lifetime: Some(lifetime),
        mutability: None,
        elem: Box::new(ty),
    })
}

/// Construct a [`syn::Type`] referring to
/// `::core::marker::PhantomData<&#lifetime ()>`.
pub(crate) fn phantom_lifetime_ty(lifetime: Lifetime) -> Type {
    let span = lifetime.span();
    let dummy = Type::Tuple(TypeTuple {
        paren_token: token::Paren::default(),
        elems: punctuated::Punctuated::default(),
    });
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("core", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("marker", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("PhantomData", span),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: token::Lt { spans: [span] },
                        args: [GenericArgument::Type(ref_ty(dummy, lifetime))]
                            .into_iter()
                            .collect(),
                        gt_token: token::Gt { spans: [span] },
                    }),
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::TypePath`] referring to
/// `<#of_ty as ::xso::FromXml>`.
fn from_xml_of(of_ty: Type) -> (Span, TypePath) {
    let span = of_ty.span();
    (
        span,
        TypePath {
            qself: Some(QSelf {
                lt_token: syn::token::Lt { spans: [span] },
                ty: Box::new(of_ty),
                position: 2,
                as_token: Some(syn::token::As { span }),
                gt_token: syn::token::Gt { spans: [span] },
            }),
            path: Path {
                leading_colon: Some(syn::token::PathSep {
                    spans: [span, span],
                }),
                segments: [
                    PathSegment {
                        ident: Ident::new("xso", span),
                        arguments: PathArguments::None,
                    },
                    PathSegment {
                        ident: Ident::new("FromXml", span),
                        arguments: PathArguments::None,
                    },
                ]
                .into_iter()
                .collect(),
            },
        },
    )
}

/// Construct a [`syn::Type`] referring to
/// `<#of_ty as ::xso::FromXml>::Builder`.
pub(crate) fn from_xml_builder_ty(of_ty: Type) -> Type {
    let (span, mut ty) = from_xml_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("Builder", span),
        arguments: PathArguments::None,
    });
    Type::Path(ty)
}

/// Construct a [`syn::Expr`] referring to
/// `<#of_ty as ::xso::FromXml>::from_events`.
pub(crate) fn from_events_fn(of_ty: Type) -> Expr {
    let (span, mut ty) = from_xml_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("from_events", span),
        arguments: PathArguments::None,
    });
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: ty.qself,
        path: ty.path,
    })
}

/// Construct a [`syn::Type`] which wraps the given `ty` in
/// `::core::option::Option<_>`.
pub(crate) fn option_ty(ty: Type) -> Type {
    let span = ty.span();
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("core", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("option", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Option", span),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: syn::token::Lt { spans: [span] },
                        args: [GenericArgument::Type(ty)].into_iter().collect(),
                        gt_token: syn::token::Gt { spans: [span] },
                    }),
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::TypePath`] referring to
/// `<#of_ty as ::xso::FromEventsBuilder>`.
fn from_events_builder_of(of_ty: Type) -> (Span, TypePath) {
    let span = of_ty.span();
    (
        span,
        TypePath {
            qself: Some(QSelf {
                lt_token: syn::token::Lt { spans: [span] },
                ty: Box::new(of_ty),
                position: 2,
                as_token: Some(syn::token::As { span }),
                gt_token: syn::token::Gt { spans: [span] },
            }),
            path: Path {
                leading_colon: Some(syn::token::PathSep {
                    spans: [span, span],
                }),
                segments: [
                    PathSegment {
                        ident: Ident::new("xso", span),
                        arguments: PathArguments::None,
                    },
                    PathSegment {
                        ident: Ident::new("FromEventsBuilder", span),
                        arguments: PathArguments::None,
                    },
                ]
                .into_iter()
                .collect(),
            },
        },
    )
}

/// Construct a [`syn::Expr`] referring to
/// `<#of_ty as ::xso::FromEventsBuilder>::feed`.
pub(crate) fn feed_fn(of_ty: Type) -> Expr {
    let (span, mut ty) = from_events_builder_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("feed", span),
        arguments: PathArguments::None,
    });
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: ty.qself,
        path: ty.path,
    })
}

fn as_xml_of(of_ty: Type) -> (Span, TypePath) {
    let span = of_ty.span();
    (
        span,
        TypePath {
            qself: Some(QSelf {
                lt_token: syn::token::Lt { spans: [span] },
                ty: Box::new(of_ty),
                position: 2,
                as_token: Some(syn::token::As { span }),
                gt_token: syn::token::Gt { spans: [span] },
            }),
            path: Path {
                leading_colon: Some(syn::token::PathSep {
                    spans: [span, span],
                }),
                segments: [
                    PathSegment {
                        ident: Ident::new("xso", span),
                        arguments: PathArguments::None,
                    },
                    PathSegment {
                        ident: Ident::new("AsXml", span),
                        arguments: PathArguments::None,
                    },
                ]
                .into_iter()
                .collect(),
            },
        },
    )
}

/// Construct a [`syn::Expr`] referring to
/// `<#of_ty as ::xso::AsXml>::as_xml_iter`.
pub(crate) fn as_xml_iter_fn(of_ty: Type) -> Expr {
    let (span, mut ty) = as_xml_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("as_xml_iter", span),
        arguments: PathArguments::None,
    });
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: ty.qself,
        path: ty.path,
    })
}

/// Construct a [`syn::Type`] referring to
/// `<#of_ty as ::xso::AsXml>::ItemIter`.
pub(crate) fn item_iter_ty(of_ty: Type, lifetime: Lifetime) -> Type {
    let (span, mut ty) = as_xml_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("ItemIter", span),
        arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
            colon2_token: None,
            lt_token: token::Lt { spans: [span] },
            args: [GenericArgument::Lifetime(lifetime)].into_iter().collect(),
            gt_token: token::Gt { spans: [span] },
        }),
    });
    Type::Path(ty)
}

/// Construct a [`syn::TypePath`] referring to `<#of_ty as IntoIterator>`.
fn into_iterator_of(of_ty: Type) -> (Span, TypePath) {
    let span = of_ty.span();
    (
        span,
        TypePath {
            qself: Some(QSelf {
                lt_token: syn::token::Lt { spans: [span] },
                ty: Box::new(of_ty),
                position: 3,
                as_token: Some(syn::token::As { span }),
                gt_token: syn::token::Gt { spans: [span] },
            }),
            path: Path {
                leading_colon: Some(syn::token::PathSep {
                    spans: [span, span],
                }),
                segments: [
                    PathSegment {
                        ident: Ident::new("core", span),
                        arguments: PathArguments::None,
                    },
                    PathSegment {
                        ident: Ident::new("iter", span),
                        arguments: PathArguments::None,
                    },
                    PathSegment {
                        ident: Ident::new("IntoIterator", span),
                        arguments: PathArguments::None,
                    },
                ]
                .into_iter()
                .collect(),
            },
        },
    )
}

/// Construct a [`syn::Type`] referring to
/// `<#of_ty as IntoIterator>::IntoIter`.
pub(crate) fn into_iterator_iter_ty(of_ty: Type) -> Type {
    let (span, mut ty) = into_iterator_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("IntoIter", span),
        arguments: PathArguments::None,
    });
    Type::Path(ty)
}

/// Construct a [`syn::Type`] referring to
/// `<#of_ty as IntoIterator>::Item`.
pub(crate) fn into_iterator_item_ty(of_ty: Type) -> Type {
    let (span, mut ty) = into_iterator_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("Item", span),
        arguments: PathArguments::None,
    });
    Type::Path(ty)
}

/// Construct a [`syn::Expr`] referring to
/// `<#of_ty as IntoIterator>::into_iter`.
pub(crate) fn into_iterator_into_iter_fn(of_ty: Type) -> Expr {
    let (span, mut ty) = into_iterator_of(of_ty);
    ty.path.segments.push(PathSegment {
        ident: Ident::new("into_iter", span),
        arguments: PathArguments::None,
    });
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: ty.qself,
        path: ty.path,
    })
}

/// Construct a [`syn::Expr`] referring to
/// `<#of_ty as ::core::iter::Extend>::extend`.
pub(crate) fn extend_fn(of_ty: Type, item_ty: Type) -> Expr {
    let span = of_ty.span();
    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: Some(QSelf {
            lt_token: syn::token::Lt { spans: [span] },
            ty: Box::new(of_ty),
            position: 3,
            as_token: Some(syn::token::As { span }),
            gt_token: syn::token::Gt { spans: [span] },
        }),
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("core", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("iter", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Extend", span),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: Some(syn::token::PathSep {
                            spans: [span, span],
                        }),
                        lt_token: syn::token::Lt { spans: [span] },
                        args: [GenericArgument::Type(item_ty)].into_iter().collect(),
                        gt_token: syn::token::Gt { spans: [span] },
                    }),
                },
                PathSegment {
                    ident: Ident::new("extend", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::TypePath`] which references the given type name.
pub(crate) fn ty_from_ident(ident: Ident) -> TypePath {
    TypePath {
        qself: None,
        path: ident.into(),
    }
}

/// Construct a [`syn::Type`] referring to `xso::OptionAsXml<#inner_ty>`.
pub(crate) fn option_as_xml_ty(inner_ty: Type) -> Type {
    let span = inner_ty.span();
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("asxml", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("OptionAsXml", span),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: token::Lt { spans: [span] },
                        args: [GenericArgument::Type(inner_ty)].into_iter().collect(),
                        gt_token: token::Gt { spans: [span] },
                    }),
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to `::xso::exports::minidom::Element`.
#[cfg(feature = "minidom")]
pub(crate) fn element_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("minidom", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Element", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Path`] referring to `::xso::UnknownAttributePolicy`.
pub(crate) fn unknown_attribute_policy_path(span: Span) -> Path {
    Path {
        leading_colon: Some(syn::token::PathSep {
            spans: [span, span],
        }),
        segments: [
            PathSegment {
                ident: Ident::new("xso", span),
                arguments: PathArguments::None,
            },
            PathSegment {
                ident: Ident::new("UnknownAttributePolicy", span),
                arguments: PathArguments::None,
            },
        ]
        .into_iter()
        .collect(),
    }
}

/// Construct a [`syn::Path`] referring to `::xso::UnknownChildPolicy`.
pub(crate) fn unknown_child_policy_path(span: Span) -> Path {
    Path {
        leading_colon: Some(syn::token::PathSep {
            spans: [span, span],
        }),
        segments: [
            PathSegment {
                ident: Ident::new("xso", span),
                arguments: PathArguments::None,
            },
            PathSegment {
                ident: Ident::new("UnknownChildPolicy", span),
                arguments: PathArguments::None,
            },
        ]
        .into_iter()
        .collect(),
    }
}

/// Construct a [`syn::Type`] referring to `::xso::fromxml::Discard`.
pub(crate) fn discard_builder_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("fromxml", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Discard", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to the built-in `bool` type.
///
/// Note that we go through `xso::exports::CoreBool` for that, because there seems
/// to be no way to access built-in types once they have been shadowed in a
/// scope.
pub(crate) fn bool_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("CoreBool", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to the built-in `u8` type.
///
/// Note that we go through `xso::exports::CoreU8` for that, because there seems
/// to be no way to access built-in types once they have been shadowed in a
/// scope.
pub(crate) fn u8_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("exports", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("CoreU8", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}

/// Construct a [`syn::Type`] referring to `::xso::fromxml::EmptyBuilder`.
pub(crate) fn empty_builder_ty(span: Span) -> Type {
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(syn::token::PathSep {
                spans: [span, span],
            }),
            segments: [
                PathSegment {
                    ident: Ident::new("xso", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("fromxml", span),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("EmptyBuilder", span),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    })
}
