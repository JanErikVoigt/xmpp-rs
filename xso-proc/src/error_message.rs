// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Infrastructure for contextual error messages

use core::fmt;

use syn::*;

/// Reference to a compound field's parent
///
/// This reference can be converted to a hopefully-useful human-readable
/// string via [`core::fmt::Display`].
#[derive(Clone, Debug)]
pub(super) enum ParentRef {
    /// The parent is addressable by a path, e.g. a struct type or enum
    /// variant.
    Named(Path),

    /// The parent is not addressable by a path, because it is in fact an
    /// ephemeral structure.
    ///
    /// Used to reference the ephemeral structures used by fields declared
    /// with `#[xml(extract(..))]`.
    Unnamed {
        /// The parent's ref.
        ///
        /// For extracts, this refers to the compound where the field with
        /// the extract is declared.
        parent: Box<ParentRef>,

        /// The field inside that parent.
        ///
        /// For extracts, this refers to the compound field where the extract
        /// is declared.
        field: Member,
    },
}

impl From<Path> for ParentRef {
    fn from(other: Path) -> Self {
        Self::Named(other)
    }
}

impl From<&Path> for ParentRef {
    fn from(other: &Path) -> Self {
        Self::Named(other.clone())
    }
}

impl fmt::Display for ParentRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Named(name) => {
                let mut first = true;
                for segment in name.segments.iter() {
                    if !first || name.leading_colon.is_some() {
                        write!(f, "::")?;
                    }
                    first = false;
                    write!(f, "{}", segment.ident)?;
                }
                write!(f, " element")
            }
            Self::Unnamed { parent, field } => {
                write!(f, "extraction for {} in {}", FieldName(field), parent)
            }
        }
    }
}

impl ParentRef {
    /// Create a new `ParentRef` for a member inside this one.
    ///
    /// Returns a [`Self::Unnamed`] with `self` as parent and `member` as
    /// field.
    pub(crate) fn child(&self, member: Member) -> Self {
        match self {
            Self::Named { .. } | Self::Unnamed { .. } => Self::Unnamed {
                parent: Box::new(self.clone()),
                field: member,
            },
        }
    }

    /// Return true if and only if this ParentRef can be addressed as a path.
    pub(crate) fn is_path(&self) -> bool {
        match self {
            Self::Named { .. } => true,
            Self::Unnamed { .. } => false,
        }
    }
}

/// Wrapper around `Path` with a more human-readable, collapsed version of
/// `Path`.
pub(crate) struct PrettyPath<'x>(pub &'x Path);

impl fmt::Display for PrettyPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = self.0.leading_colon.is_none();
        for segment in self.0.segments.iter() {
            if !first {
                f.write_str("::")?;
            }
            write!(f, "{}", segment.ident)?;
            first = false;
        }
        Ok(())
    }
}

/// Ephemeral struct to create a nice human-readable representation of
/// [`syn::Member`].
///
/// It implements [`core::fmt::Display`] for that purpose and is otherwise of
/// little use.
#[repr(transparent)]
pub(crate) struct FieldName<'x>(pub &'x Member);

impl fmt::Display for FieldName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Member::Named(v) => write!(f, "field '{}'", v),
            Member::Unnamed(v) => write!(f, "unnamed field {}", v.index),
        }
    }
}

/// Create a string error message for a missing attribute.
///
/// `parent_name` should point at the compound which is being parsed and
/// `field` should be the field to which the attribute belongs.
pub(super) fn on_missing_attribute(parent_name: &ParentRef, field: &Member) -> String {
    format!(
        "Required attribute {} on {} missing.",
        FieldName(field),
        parent_name
    )
}

/// Create a string error message for a missing child element.
///
/// `parent_name` should point at the compound which is being parsed and
/// `field` should be the field to which the child belongs.
pub(super) fn on_missing_child(parent_name: &ParentRef, field: &Member) -> String {
    format!("Missing child {} in {}.", FieldName(field), parent_name)
}

/// Create a string error message for a duplicate child element.
///
/// `parent_name` should point at the compound which is being parsed and
/// `field` should be the field to which the child belongs.
pub(super) fn on_duplicate_child(parent_name: &ParentRef, field: &Member) -> String {
    format!(
        "{} must not have more than one child in {}.",
        parent_name,
        FieldName(field)
    )
}
