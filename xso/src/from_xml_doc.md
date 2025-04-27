# Make a struct or enum parseable from XML

This derives the [`FromXml`] trait on a struct or enum. It is the counterpart
to [`macro@AsXml`].

## Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo;

let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example'/>").unwrap();
assert_eq!(foo, Foo);
```

## Table of contents

1. [Attributes](#attributes)
2. [Struct meta](#struct-meta)
3. [Enums](#enums)
    1. [Name-switched enum meta](#name-switched-enum-meta)
    2. [Dynamic enum meta](#dynamic-enum-meta)
4. [Field meta](#field-meta)
    1. [`attribute` meta](#attribute-meta)
    2. [`child` meta](#child-meta)
    3. [`element` meta](#element-meta)
    4. [`extract` meta](#extract-meta)
    5. [`flag` meta](#flag-meta)
    6. [`lang` meta](#lang-meta)
    6. [`text` meta](#text-meta)

## Attributes

The derive macros need additional information, such as XML namespaces and
names to match. This must be specified via key-value pairs on the type or
fields the derive macro is invoked on. These key-value pairs are specified as
Rust attributes. In order to disambiguate between XML attributes and Rust
attributes, we are going to refer to Rust attributes using the term *meta*
instead, which is consistent with the Rust language reference calling that
syntax construct *meta*.

All key-value pairs interpreted by these derive macros must be wrapped in a
`#[xml( ... )]` *meta*.

The values associated with the keys may be of different types, defined as
such:

- *path*: A Rust path, like `some_crate::foo::Bar`. Note that `foo` on its own
  is also a path.
- *string literal*: A string literal, like `"hello world!"`.
- *type*: A Rust type.
- *expression*: A Rust expression.
- *ident*: A single Rust identifier.
- *nested*: The meta is followed by parentheses, inside of which meta-specific
  additional keys are present.
- flag: Has no value. The key's mere presence has relevance and it must not be
  followed by a `=` sign.

## Struct meta

The following keys are defined on structs:

| Key | Value type | Description |
| --- | --- | --- |
| `namespace` | *string literal* or *path* | The XML element namespace to match. If it is a *path*, it must point at a `&'static str`. |
| `name` | *string literal* or *path* | The XML element name to match. If it is a *path*, it must point at a `&'static NcNameStr`. |
| `transparent` | *flag* | If present, declares the struct as *transparent* struct (see below) |
| `builder` | optional *ident* | The name to use for the generated builder type. |
| `iterator` | optional *ident* | The name to use for the generated iterator type. |
| `on_unknown_attribute` | optional *ident* | Name of an [`UnknownAttributePolicy`] member, controlling how unknown attributes are handled. |
| `on_unknown_child` | optional *ident* | Name of an [`UnknownChildPolicy`] member, controlling how unknown children are handled. |
| `discard` | optional *nested* | Contains field specifications of content to ignore. See below for details. |
| `deserialize_callback` | optional *path* | Path to a `fn(&mut T) -> Result<(), Error>` which is called on the deserialized struct after deserialization. |

Note that the `name` value must be a valid XML element name, without colons.
The namespace prefix, if any, is assigned automatically at serialisation time
and cannot be overridden. The following will thus not compile:

```compile_fail
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "fnord:foo")]  // colon not allowed
struct Foo;
```

If `builder` or `iterator` are given, the respective generated types will use
the given names instead of names chosen by the derive macro implementation.
Helper types will use these names as prefix. The exact names of helper types
are implementation defined, which is why any type name starting with the
identifiers passed to either of these keys is considered reserved.

By default, the builder type uses the type's name suffixed with
`FromXmlBuilder` and the iterator type uses the type's name suffixed with
`AsXmlIterator`.

If the struct is marked as `transparent`, it must not have a `namespace` or
`name` set and it must have exactly one field. That field's type must
implement [`FromXml`] in order to derive `FromXml` and [`AsXml`] in order to
derive `AsXml`. The struct will be (de-)serialised exactly like the type of
that single field. This allows a newtype-like pattern for XSO structs.

`discard` may contain zero or more field meta which describe XML content to
silently ignore. The syntax is the same as within the `#[xml(..)]` meta used
on fields, however, any parameters which aren't strictly needed to match the
content are rejected (for example, you cannot set the codec on a discarded
attribute because it is irrelevant). Discarded content is never emitted during
serialisation. Its absence does not cause errors.

```
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo", discard(text))]
struct Foo;
```

## Enums

Two different `enum` flavors are supported:

1. [**Name-switched enums**](#name-switched-enum-meta) have a fixed XML
   namespace they match on and each variant corresponds to a different XML
   element name within that namespace.

2. [**Dynamic enums**](#dynamic-enum-meta) have entirely unrelated variants.

At the source-code level, they are distinguished by the meta keys which are
present on the `enum`: The different variants have different sets of mandatory
keys and can thus be uniquely identified.

### Name-switched enum meta

Name-switched enums match a fixed XML namespace and then select the enum
variant based on the XML element's name. Name-switched enums are declared by
setting the `namespace` key on a `enum` item.

The following keys are defined on name-switched enums:

| Key | Value type | Description |
| --- | --- | --- |
| `namespace` | *string literal* or *path* | The XML element namespace to match for this enum. If it is a *path*, it must point at a `&'static str`. |
| `builder` | optional *ident* | The name to use for the generated builder type. |
| `iterator` | optional *ident* | The name to use for the generated iterator type. |
| `exhaustive` | *flag* | If present, the enum considers itself authoritative for its namespace; unknown elements within the namespace are rejected instead of treated as mismatch. |
| `discard` | optional *nested* | Contains field specifications of content to ignore. See the struct meta docs for details. |
| `deserialize_callback` | optional *path* | Path to a `fn(&mut T) -> Result<(), Error>` which is called on the deserialized enum after deserialization. |

All variants of a name-switched enum live within the same namespace and are
distinguished exclusively by their XML name within that namespace. The
contents of the XML element (including attributes) is not inspected before
selecting the variant when parsing XML.

If *exhaustive* is set and an element is encountered which matches the
namespace of the enum, but matches none of its variants, parsing will fail
with an error. If *exhaustive* is *not* set, in such a situation, parsing
would attempt to continue with other siblings of the enum, attempting to find
a handler for that element.

Note that the *exhaustive* flag is orthogonal to the Rust attribute
`#[non_exhaustive]`.

For details on `builder` and `iterator`, see the [Struct meta](#struct-meta)
documentation above.

#### Name-switched enum variant meta

| Key | Value type | Description |
| --- | --- | --- |
| `name` | *string literal* or *path* | The XML element name to match for this variant. If it is a *path*, it must point at a `&'static NcNameStr`. |
| `on_unknown_attribute` | optional *ident* | Name of an [`UnknownAttributePolicy`] member, controlling how unknown attributes are handled. |
| `on_unknown_child` | optional *ident* | Name of an [`UnknownChildPolicy`] member, controlling how unknown children are handled. |

Note that the `name` value must be a valid XML element name, without colons.
The namespace prefix, if any, is assigned automatically at serialisation time
and cannot be overridden.

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example")]
enum Foo {
    #[xml(name = "a")]
    Variant1 {
        #[xml(attribute)]
        foo: String,
    },
    #[xml(name = "b")]
    Variant2 {
        #[xml(attribute)]
        bar: String,
    },
}

let foo: Foo = xso::from_bytes(b"<a xmlns='urn:example' foo='hello'/>").unwrap();
assert_eq!(foo, Foo::Variant1 { foo: "hello".to_string() });

let foo: Foo = xso::from_bytes(b"<b xmlns='urn:example' bar='hello'/>").unwrap();
assert_eq!(foo, Foo::Variant2 { bar: "hello".to_string() });
```

### Dynamic enum meta

Dynamic enums select their variants by attempting to match them in declaration
order. Dynamic enums are declared by not setting the `namespace` key on an
`enum` item.

The following keys are defined on dynamic enums:

| Key | Value type | Description |
| --- | --- | --- |
| `builder` | optional *ident* | The name to use for the generated builder type. |
| `iterator` | optional *ident* | The name to use for the generated iterator type. |
| `discard` | optional *nested* | Contains field specifications of content to ignore. See the struct meta docs for details. |
| `deserialize_callback` | optional *path* | Path to a `fn(&mut T) -> Result<(), Error>` which is called on the deserialized enum after deserialization. |

For details on `builder` and `iterator`, see the [Struct meta](#struct-meta)
documentation above.

#### Dynamic enum variant meta

Dynamic enum variants are completely independent of one another and thus use
the same meta structure as structs. See [Struct meta](#struct-meta) for
details.

The `builder`, `iterator` and `debug` keys cannot be used on dynamic enum
variants.

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml()]
enum Foo {
    #[xml(namespace = "urn:example:ns1", name = "a")]
    Variant1 {
        #[xml(attribute)]
        foo: String,
    },
    #[xml(namespace = "urn:example:ns2", name = "b")]
    Variant2 {
        #[xml(attribute)]
        bar: String,
    },
}

let foo: Foo = xso::from_bytes(b"<a xmlns='urn:example:ns1' foo='hello'/>").unwrap();
assert_eq!(foo, Foo::Variant1 { foo: "hello".to_string() });

let foo: Foo = xso::from_bytes(b"<b xmlns='urn:example:ns2' bar='hello'/>").unwrap();
assert_eq!(foo, Foo::Variant2 { bar: "hello".to_string() });
```

## Field meta

For fields, the *meta* consists of a nested meta inside the `#[xml(..)]` meta,
the identifier of which controls *how* the field is mapped to XML, while the
contents control the parameters of that mapping.

The following mapping types are defined:

| Type | Description |
| --- | --- |
| [`attribute`](#attribute-meta) | Map the field to an XML attribute on the struct's element |
| [`child`](#child-meta) | Map the field to a child element |
| [`element`](#element-meta) | Map the field to a child element as [`minidom::Element`] |
| [`extract`](#extract-meta) | Map the field to contents of a child element of specified structure |
| [`text`](#text-meta) | Map the field to the text content of the struct's element |

#### Field order

Field order **matters**. The fields are parsed in the order they are declared
(for children, anyway). If multiple fields match a given child element, the
first field which matches will be taken. The only exception is
`#[xml(element(n = ..))]`, which is always processed last.

When XML is generated from a struct, the child elements are also generated
in the order of the fields. That means that passing an XML element through
`FromXml` and `AsXml` may re-order some child elements.

Sorting order between elements which match the same field is generally
preserved, if the container preserves sort order on insertion.

### `attribute` meta

The `attribute` meta causes the field to be mapped to an XML attribute of the
same name. For `FromXml`, the field's type must implement [`FromXmlText`] and
for `AsXml`, the field's type must implement [`AsOptionalXmlText`].

The following keys can be used inside the `#[xml(attribute(..))]` meta:

| Key | Value type | Description |
| --- | --- | --- |
| `namespace` | optional *string literal* or *path* | The namespace of the XML attribute to match. If it is a *path*, it must point at a `&'static str`. Note that attributes, unlike elements, are unnamespaced by default. |
| `name` | optional *string literal* or *path* | The name of the XML attribute to match. If it is a *path*, it must point at a `&'static NcNameStr`. |
| `default` | *flag* | If present, an absent attribute will substitute the default value instead of raising an error. |
| `type_` | *type* | Optional explicit type specification. Only allowed within `#[xml(extract(fields(..)))]`. |
| `codec` | optional *expression* | [`TextCodec`] implementation which is used to encode or decode the field. |

If the `name` key contains a namespace prefix, it must be one of the prefixes
defined as built-in in the XML specifications. That prefix will then be
expanded to the corresponding namespace URI and the value for the `namespace`
key is implied. Mixing a prefixed name with an explicit `namespace` key is
not allowed.

The `attribute` meta also supports a shorthand syntax,
`#[xml(attribute = ..)]`, where the value is treated as the value for the
`name` key (with optional prefix as described above, and unnamespaced
otherwise).

If `default` is specified and the attribute is absent in the source, the value
is generated using [`core::default::Default`], requiring the field type to
implement the `Default` trait for a `FromXml` derivation. `default` has no
influence on `AsXml`.

If `type_` is specified and the `attribute` meta is used within an
`#[xml(extract(fields(..)))]` meta, the specified type is used instead of the
field type on which the `extract` is declared.

If `codec` is given, the given `codec` value must implement
[`TextCodec<T>`][`TextCodec`] where `T` is the type of the field.

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo {
    #[xml(attribute)]
    a: String,
    #[xml(attribute = "bar")]
    b: String,
    #[xml(attribute(name = "baz"))]
    c: String,
    #[xml(attribute(namespace = "urn:example", name = "fnord"))]
    d: String,
    #[xml(attribute = "xml:lang")]
    e: String,
};

let foo: Foo = xso::from_bytes(b"<foo
    xmlns='urn:example'
    a='1' bar='2' baz='3'
    xmlns:tns0='urn:example' tns0:fnord='4'
    xml:lang='5'
/>").unwrap();
assert_eq!(foo, Foo {
    a: "1".to_string(),
    b: "2".to_string(),
    c: "3".to_string(),
    d: "4".to_string(),
    e: "5".to_string(),
});
```

Note that it is not possible to have two `#[xml(attribute)]` fields which
match the same XML attribute:

```compile_fail
# use xso::FromXml;
#[derive(FromXml)]
#[xml(namespace = "urn:example", name = "dup")]
struct Dup {
    #[xml(attribute)]
    a: String,

    #[xml(attribute = "a")]
    b: String,
}
```

```compile_fail
# use xso::FromXml;
static A: &str = "a";

#[derive(FromXml)]
#[xml(namespace = "urn:example", name = "dup")]
struct Dup {
    #[xml(attribute)]
    a: String,

    #[xml(attribute = A)]
    b: String,
}
```

### `child` meta

The `child` meta causes the field to be mapped to a child element of the
element.

The following keys can be used inside the `#[xml(child(..))]` meta:

| Key | Value type | Description |
| --- | --- | --- |
| `default` | *flag* | If present, an absent child will substitute the default value instead of raising an error. |
| `n` | `1` or `..` | If `1`, a single element is parsed. If `..`, a collection is parsed. Defaults to `1`. |

When parsing a single child element (i.e. `n = 1` or no `n` value set at all),
the field's type must implement [`FromXml`] in order to derive `FromXml` and
[`AsXml`] in order to derive `AsXml`.

When parsing a collection (with `n = ..`), the field's type must implement
[`IntoIterator<Item = T>`][`core::iter::IntoIterator`], where `T` must
implement [`FromXml`] in order to derive `FromXml` and [`AsXml`] in order to
derive `AsXml`. In addition, the field's type must implement
[`Extend<T>`][`core::iter::Extend`] to derive `FromXml` and the field's
reference type must implement `IntoIterator<Item = &'_ T>` to derive `AsXml`.

If `default` is specified and the child is absent in the source, the value
is generated using [`core::default::Default`], requiring the field type to
implement the `Default` trait for a `FromXml` derivation. `default` has no
influence on `AsXml`. Combining `default` and `n` where `n` is not set to `1`
is not supported and will cause a compile-time error.

Using `default` with a type other than `Option<T>` will cause the
serialisation to mismatch the deserialisation (i.e. the struct is then not
roundtrip-safe), because the deserialisation does not compare the value
against `default` (but has special provisions to work with `Option<T>`).

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "child")]
struct Child {
    #[xml(attribute = "some-attr")]
    some_attr: String,
}

#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "other-child")]
struct OtherChild {
    #[xml(attribute = "some-attr")]
    some_attr: String,
}

#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "parent")]
struct Parent {
    #[xml(attribute)]
    foo: String,

    #[xml(child)]
    bar: Child,

    #[xml(child(n = ..))]
    baz: Vec<OtherChild>,
}

let parent: Parent = xso::from_bytes(b"<parent
    xmlns='urn:example'
    foo='hello world!'
><child
    some-attr='within'
/><other-child
    some-attr='c1'
/><other-child
    some-attr='c2'
/></parent>").unwrap();
assert_eq!(parent, Parent {
    foo: "hello world!".to_owned(),
    bar: Child { some_attr: "within".to_owned() },
    baz: vec! [
        OtherChild { some_attr: "c1".to_owned() },
        OtherChild { some_attr: "c2".to_owned() },
    ],
});
```

### `element` meta

The `element` meta causes the field to be mapped to child elements, stored as
a container containing [`minidom::Element`] instances.

This meta is only available if `xso` is being built with the `"minidom"`
feature.

The following keys can be used inside the `#[xml(extract(..))]` meta:

| Key | Value type | Description |
| --- | --- | --- |
| `default` | flag | If present, an absent child will substitute the default value instead of raising an error. |
| `n` | `1` or `..` | If `1`, a single element is parsed. If `..`, a collection is parsed. Defaults to `1`. |

When parsing a single child element (i.e. `n = 1` or no `n` value set at all),
the field's type must be a `minidom::Element`.

When parsing a collection (with `n = ..`), the field's type must be a
collection of `minidom::Element`. It must thus implement
[`IntoIterator<Item = minidom::Element>`][`core::iter::IntoIterator`]. In
addition, the field's type must implement
[`Extend<minidom::Element>`][`core::iter::Extend`] to derive `FromXml` and the
field's reference type must implement
`IntoIterator<Item = &'_ minidom::Element>` to derive `AsXml`.

If `default` is specified and the child is absent in the source, the value
is generated using [`core::default::Default`]. `default` has no influence on
`AsXml`. Combining `default` and `n` where `n` is not set to `1` is not
supported and will cause a compile-time error.

Using `default` with a type other than `Option<T>` will cause the
serialisation to mismatch the deserialisation (i.e. the struct is then not
roundtrip-safe), because the deserialisation does not compare the value
against `default` (but has special provisions to work with `Option<T>`).

Fields with the `element` meta are deserialised with the lowest priority.
While other fields are processed in the order they are declared, `element`
fields may capture arbitrary child elements, so they are considered as the
last choice when no other field matched a given child element. In addition,
it is not allowed to have more than one field in any given struct with the
`#[xml(element)]` meta.

#### Example

```rust
# #[cfg(feature = "minidom")]
# {
# use xso::FromXml;
# use xso::exports::minidom;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "parent")]
struct Parent {
    #[xml(element(n = ..))]
    misc: Vec<minidom::Element>,
}

let parent: Parent = xso::from_bytes(b"<parent
    xmlns='urn:example'
><child-a/><child-b/><child-a/></parent>").unwrap();
assert_eq!(parent.misc[0].name(), "child-a");
assert_eq!(parent.misc[1].name(), "child-b");
assert_eq!(parent.misc[2].name(), "child-a");
# }
```

### `extract` meta

The `extract` meta causes the field to be mapped to the *contents* of a child
element.

The following keys can be used inside the `#[xml(extract(..))]` meta:

| Key | Value type | Description |
| --- | --- | --- |
| `namespace` | optional *string literal* or *path* | The XML namespace of the child element. |
| `name` | optional *string literal* or *path* | The XML name of the child element. If it is a *path*, it must point at a `&'static NcNameStr`. |
| `default` | *flag* | If present, an absent child will substitute the default value instead of raising an error. |
| `n` | `1` or `..` | If `1`, a single element is parsed. If `..`, a collection is parsed. Defaults to `1`. |
| `fields` | *nested* | A list of [field meta](#field-meta) which describe the contents of the child element. |
| `on_unknown_attribute` | optional *ident* | Name of an [`UnknownAttributePolicy`] member, controlling how unknown attributes are handled. |
| `on_unknown_child` | optional *ident* | Name of an [`UnknownChildPolicy`] member, controlling how unknown children are handled. |

If the `name` key contains a namespace prefix, it must be one of the prefixes
defined as built-in in the XML specifications. That prefix will then be
expanded to the corresponding namespace URI and the value for the `namespace`
key is implied. Mixing a prefixed name with an explicit `namespace` key is
not allowed.

Both `namespace` and `name` may be omitted. If `namespace` is omitted, it
defaults to the namespace of the surrounding container. If `name` is omitted
and the `extract` meta is being used on a named field, that field's name is
used. If `name` is omitted and `extract` is not used on a named field, an
error is emitted.

When parsing a single child element (i.e. `n = 1` or no `n` value set at all),
the extracted field's type is set to be the same type as the field on which
the extract is declared, unless overridden in the extracted field's meta.

When parsing a collection (with `n = ..`), the extracted fields within
`fields()` must all have type specifications. Not all fields kinds support
that.

The sequence of field meta inside `fields` can be thought of as a nameless
tuple-style struct. The macro generates serialisation/deserialisation code
for that nameless tuple-style struct and uses it to serialise/deserialise
the field.

If `default` is specified and the child is absent in the source, the value
is generated using [`core::default::Default`], requiring the field type to
implement the `Default` trait for a `FromXml` derivation. `default` has no
influence on `AsXml`. Combining `default` and `n` where `n` is not set to `1`
is not supported and will cause a compile-time error.

Mixing `default` on the `#[xml(extract)]` itself with `default` on the
extracted inner fields creates non-roundtrip-safe parsing, unless you also
use twice-nested [`core::option::Option`] types. That means that when
deserialising a piece of XML and reserialising it without changing the
contents of the struct in Rust, the resulting XML may not match the input.
This is because to the serialiser, if only one layer of
[`core::option::Option`] is used, it is not possible to distinguish which of
the two layers were defaulted. The exact behaviour on serialisation in such a
situation is *not guaranteed* and may change between versions of the `xso`
crate, its dependencies, the standard library, or even rustc itself.

Using `default` with a type other than `Option<T>` will cause the
serialisation to mismatch the deserialisation, too (i.e. the struct is then
not roundtrip-safe), because the deserialisation does not compare the value
against `default` (but has special provisions to work with `Option<T>`).

If more than one single field is contained in `fields`, the fields will be
extracted as a tuple in the order they are given in the meta. In addition, it
is required to explicitly specify each extracted field's type in that case.

Using `extract` instead of `child` combined with a specific struct declaration
comes with trade-offs. On the one hand, using `extract` gives you flexibility
in regard of the specific serialisation of a field: it is possible to exchange
a nested child element for an attribute without changing the Rust interface
of the struct.

On the other hand, `extract` meta declarations can quickly become unwieldy
and they may not support all configuration options which may in the future be
added on structs (such as configuring handling of undeclared attributes) and
they cannot be used for enumerations.

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo {
    #[xml(extract(namespace = "urn:example", name = "bar", fields(attribute = "a")))]
    a: String,
}

let foo: Foo = xso::from_bytes(b"<foo
    xmlns='urn:example'><bar a='xyz'/></foo>").unwrap();
assert_eq!(foo, Foo {
    a: "xyz".to_string(),
});
```

### `flag` meta

The `flag` meta causes the field to be mapped to a single, optional child element. Absence of the child is equivalent to the value `false`, presence
of the child element is equivalent to the value `true`.

The following keys can be used inside the `#[xml(flag(..))]` meta:

| Key | Value type | Description |
| --- | --- | --- |
| `namespace` | optional *string literal* or *path* | The namespace of the XML element to match. If it is a *path*, it must point at a `&'static str`. |
| `name` | optional *string literal* or *path* | The name of the XML element to match. If it is a *path*, it must point at a `&'static NcNameStr`. |

The field on which the `flag` meta is used must be of type `bool`.

Both `namespace` and `name` may be omitted. If `namespace` is omitted, it
defaults to the namespace of the surrounding container. If `name` is omitted
and the `flag` meta is being used on a named field, that field's name is
used. If `name` is omitted and `flag` is not used on a named field, an
error is emitted.

When parsing, any contents within the child element generate a parse error.

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo {
    #[xml(flag(name = "flag"))]
    flag: bool,
};

let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example'><flag/></foo>").unwrap();
assert_eq!(foo, Foo {
    flag: true,
});

let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example'/>").unwrap();
assert_eq!(foo, Foo {
    flag: false,
});
```

### `lang` meta

The `lang` meta allows to access the (potentially inherited) logical
`xml:lang` value as defined in
[XML 1.0 § 2.12](https://www.w3.org/TR/REC-xml/#sec-lang-tag).

This meta supports no arguments and can only be used on fields of type
`Option<String>`.

Unlike `#[xml(attribute = "xml:lang")]`, the `#[xml(lang)]` meta takes
inheritance into account.

**Note:** Using this meta is not roundtrip-safe. `rxml` will always emit its
value on serialisation, even if it was inherited during deserialisation.

#### Example

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "bar")]
struct Bar {
    #[xml(lang)]
    lang: Option<String>,
};

#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo {
    #[xml(child)]
    child: Bar,
};

// `xml:lang` gets inherited from <foo/> to <bar/>
let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example' xml:lang='en'><bar/></foo>").unwrap();
assert_eq!(foo, Foo {
    child: Bar {
        lang: Some("en".to_owned()),
    },
});

// `xml:lang` gets set/overwritten in <bar/>
let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example' xml:lang='en'><bar xml:lang='de'/></foo>").unwrap();
assert_eq!(foo, Foo {
    child: Bar {
        lang: Some("de".to_owned()),
    },
});
```

Note that it is not possible to use `#[xml(lang)]` and an `#[xml(attribute)]`
which also matches `xml:lang` in the same struct:

```compile_fail
# use xso::FromXml;
# use xso::exports::rxml::XMLNS_XML;
#[derive(FromXml)]
#[xml(namespace = "urn:example", name = "dup")]
struct Dup {
    #[xml(attribute(namespace = XMLNS_XML, name = "lang"))]
    a: String,

    #[xml(lang)]
    b: Option<String>,
}
```

### `text` meta

The `text` meta causes the field to be mapped to the text content of the
element.

| Key | Value type | Description |
| --- | --- | --- |
| `codec` | optional *expression* | [`TextCodec`] implementation which is used to encode or decode the field. |
| `type_` | optional *type* | Explicit type specification. Only allowed within `#[xml(extract(fields(..)))]`. |

If `codec` is given, the given `codec` value must implement
[`TextCodec<T>`][`TextCodec`] where `T` is the type of the field.

If `codec` is *not* given, the field's type must implement [`FromXmlText`] for
`FromXml` and for `AsXml`, the field's type must implement [`AsXmlText`].

If `type_` is specified and the `text` meta is used within an
`#[xml(extract(fields(..)))]` meta, the specified type is used instead of the
field type on which the `extract` is declared.

The `text` meta also supports a shorthand syntax, `#[xml(text = ..)]`, where
the value is treated as the value for the `codec` key (with optional prefix as
described above, and unnamespaced otherwise).

Only a single field per struct may be annotated with `#[xml(text)]` at a time,
to avoid parsing ambiguities. This is also true if only `AsXml` is derived on
a field, for consistency.

#### Example without codec

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo {
    #[xml(text)]
    a: String,
};

let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example'>hello</foo>").unwrap();
assert_eq!(foo, Foo {
    a: "hello".to_string(),
});
```

#### Example with codec

```rust
# use xso::FromXml;
#[derive(FromXml, Debug, PartialEq)]
#[xml(namespace = "urn:example", name = "foo")]
struct Foo {
    #[xml(text = xso::text::EmptyAsNone)]
    a: Option<String>,
};

let foo: Foo = xso::from_bytes(b"<foo xmlns='urn:example'/>").unwrap();
assert_eq!(foo, Foo {
    a: None,
});
```
