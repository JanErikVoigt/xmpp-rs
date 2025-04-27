// Copyright (c) 2024 Jonas Schäfer <jonas@zombofant.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Utilities which may eventually move upstream to the `rxml` crate.

use alloc::borrow::{Cow, ToOwned};

use rxml::{parser::EventMetrics, AttrMap, Event, Namespace, NcName, NcNameStr, XmlVersion};

/// # Serialisable piece of XML
///
/// This item represents a piece of an XML document which can be serialised
/// into bytes by converting it to an [`rxml::Item`] and then feeding it to a
/// [`rxml::Encoder`].
///
/// Unlike `rxml::Item`, the contents of this item may either be owned or
/// borrowed, individually. This enables the use in an [`crate::AsXml`] trait
/// even if data needs to be generated during serialisation.
#[derive(Debug)]
pub enum Item<'x> {
    /// XML declaration
    XmlDeclaration(XmlVersion),

    /// Start of an element header
    ElementHeadStart(
        /// Namespace name
        Namespace,
        /// Local name of the attribute
        Cow<'x, NcNameStr>,
    ),

    /// An attribute key/value pair
    Attribute(
        /// Namespace name
        Namespace,
        /// Local name of the attribute
        Cow<'x, NcNameStr>,
        /// Value of the attribute
        Cow<'x, str>,
    ),

    /// End of an element header
    ElementHeadEnd,

    /// A piece of text (in element content, not attributes)
    Text(Cow<'x, str>),

    /// Footer of an element
    ///
    /// This can be used either in places where [`Text`] could be used to
    /// close the most recently opened unclosed element, or it can be used
    /// instead of [`ElementHeadEnd`] to close the element using `/>`, without
    /// any child content.
    ///
    ///   [`Text`]: Self::Text
    ///   [`ElementHeadEnd`]: Self::ElementHeadEnd
    ElementFoot,
}

impl Item<'_> {
    /// Exchange all borrowed pieces inside this item for owned items, cloning
    /// them if necessary.
    pub fn into_owned(self) -> Item<'static> {
        match self {
            Self::XmlDeclaration(v) => Item::XmlDeclaration(v),
            Self::ElementHeadStart(ns, name) => {
                Item::ElementHeadStart(ns, Cow::Owned(name.into_owned()))
            }
            Self::Attribute(ns, name, value) => Item::Attribute(
                ns,
                Cow::Owned(name.into_owned()),
                Cow::Owned(value.into_owned()),
            ),
            Self::ElementHeadEnd => Item::ElementHeadEnd,
            Self::Text(value) => Item::Text(Cow::Owned(value.into_owned())),
            Self::ElementFoot => Item::ElementFoot,
        }
    }

    /// Return an [`rxml::Item`], which borrows data from this item.
    pub fn as_rxml_item(&self) -> rxml::Item<'_> {
        match self {
            Self::XmlDeclaration(ref v) => rxml::Item::XmlDeclaration(*v),
            Self::ElementHeadStart(ref ns, ref name) => rxml::Item::ElementHeadStart(ns, name),
            Self::Attribute(ref ns, ref name, ref value) => rxml::Item::Attribute(ns, name, value),
            Self::ElementHeadEnd => rxml::Item::ElementHeadEnd,
            Self::Text(ref value) => rxml::Item::Text(value),
            Self::ElementFoot => rxml::Item::ElementFoot,
        }
    }
}

/// Iterator adapter which converts an iterator over [`Event`][`rxml::Event`]
/// to an iterator over [`Item<'static>`][`Item`].
///
/// This iterator consumes the events and returns items which contain the data
/// in an owned fashion.
#[cfg(feature = "minidom")]
pub(crate) struct EventToItem<I> {
    inner: I,
    attributes: Option<rxml::xml_map::IntoIter<alloc::string::String>>,
}

#[cfg(feature = "minidom")]
impl<I> EventToItem<I> {
    pub(crate) fn new(inner: I) -> Self {
        Self {
            inner,
            attributes: None,
        }
    }

    fn drain(&mut self) -> Option<Item<'static>> {
        match self.attributes {
            Some(ref mut attrs) => {
                if let Some(((ns, name), value)) = attrs.next() {
                    Some(Item::Attribute(ns, Cow::Owned(name), Cow::Owned(value)))
                } else {
                    self.attributes = None;
                    Some(Item::ElementHeadEnd)
                }
            }
            None => None,
        }
    }

    fn update(&mut self, ev: Event) -> Item<'static> {
        assert!(self.attributes.is_none());
        match ev {
            Event::XmlDeclaration(_, v) => Item::XmlDeclaration(v),
            Event::StartElement(_, (ns, name), attrs) => {
                self.attributes = Some(attrs.into_iter());
                Item::ElementHeadStart(ns, Cow::Owned(name))
            }
            Event::Text(_, value) => Item::Text(Cow::Owned(value)),
            Event::EndElement(_) => Item::ElementFoot,
        }
    }
}

#[cfg(feature = "minidom")]
impl<I: Iterator<Item = Result<Event, crate::error::Error>>> Iterator for EventToItem<I> {
    type Item = Result<Item<'static>, crate::error::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.drain() {
            return Some(Ok(item));
        }
        let next = match self.inner.next() {
            Some(Ok(v)) => v,
            Some(Err(e)) => return Some(Err(e)),
            None => return None,
        };
        Some(Ok(self.update(next)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // we may create an indefinte amount of items for a single event,
        // so we cannot provide a reasonable upper bound.
        (self.inner.size_hint().0, None)
    }
}

/// Iterator adapter which converts an iterator over [`Item`] to
/// an iterator over [`Event`][`crate::Event`].
///
/// As `Event` does not support borrowing data, this iterator copies the data
/// from the items on the fly.
pub(crate) struct ItemToEvent<I> {
    inner: I,
    event_buffer: Option<Event>,
    elem_buffer: Option<(Namespace, NcName, AttrMap)>,
}

impl<'x, I: Iterator<Item = Result<Item<'x>, crate::error::Error>>> ItemToEvent<I> {
    /// Create a new adapter with `inner` as the source iterator.
    pub(crate) fn new(inner: I) -> Self {
        Self {
            inner,
            event_buffer: None,
            elem_buffer: None,
        }
    }
}

impl<I> ItemToEvent<I> {
    fn update(&mut self, item: Item<'_>) -> Result<Option<Event>, crate::error::Error> {
        assert!(self.event_buffer.is_none());
        match item {
            Item::XmlDeclaration(v) => {
                assert!(self.elem_buffer.is_none());
                Ok(Some(Event::XmlDeclaration(EventMetrics::zero(), v)))
            }
            Item::ElementHeadStart(ns, name) => {
                if self.elem_buffer.is_some() {
                    // this is only used with AsXml implementations, so
                    // triggering this is always a coding failure instead of a
                    // runtime error.
                    panic!("got a second ElementHeadStart items without ElementHeadEnd inbetween: ns={:?} name={:?} (state={:?})", ns, name, self.elem_buffer);
                }
                self.elem_buffer = Some((ns.to_owned(), name.into_owned(), AttrMap::new()));
                Ok(None)
            }
            Item::Attribute(ns, name, value) => {
                let Some((_, _, attrs)) = self.elem_buffer.as_mut() else {
                    // this is only used with AsXml implementations, so
                    // triggering this is always a coding failure instead of a
                    // runtime error.
                    panic!(
                        "got a second Attribute item without ElementHeadStart: ns={:?}, name={:?}",
                        ns, name
                    );
                };
                attrs.insert(ns, name.into_owned(), value.into_owned());
                Ok(None)
            }
            Item::ElementHeadEnd => {
                let Some((ns, name, attrs)) = self.elem_buffer.take() else {
                    // this is only used with AsXml implementations, so
                    // triggering this is always a coding failure instead of a
                    // runtime error.
                    panic!(
                        "got ElementHeadEnd item without ElementHeadStart: {:?}",
                        item
                    );
                };
                Ok(Some(Event::StartElement(
                    EventMetrics::zero(),
                    (ns, name),
                    attrs,
                )))
            }
            Item::Text(value) => {
                if let Some(elem_buffer) = self.elem_buffer.as_ref() {
                    // this is only used with AsXml implementations, so
                    // triggering this is always a coding failure instead of a
                    // runtime error.
                    panic!("got Text after ElementHeadStart but before ElementHeadEnd: Text({:?}) (state = {:?})", value, elem_buffer);
                }
                Ok(Some(Event::Text(EventMetrics::zero(), value.into_owned())))
            }
            Item::ElementFoot => {
                let end_ev = Event::EndElement(EventMetrics::zero());
                let result = if let Some((ns, name, attrs)) = self.elem_buffer.take() {
                    // content-less element
                    self.event_buffer = Some(end_ev);
                    Event::StartElement(EventMetrics::zero(), (ns, name), attrs)
                } else {
                    end_ev
                };
                Ok(Some(result))
            }
        }
    }
}

impl<'x, I: Iterator<Item = Result<Item<'x>, crate::error::Error>>> Iterator for ItemToEvent<I> {
    type Item = Result<Event, crate::error::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.event_buffer.take() {
            return Some(Ok(event));
        }
        loop {
            let item = match self.inner.next() {
                Some(Ok(v)) => v,
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };
            if let Some(v) = self.update(item).transpose() {
                return Some(v);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // we may create an indefinte amount of items for a single event,
        // so we cannot provide a reasonable upper bound.
        (self.inner.size_hint().0, None)
    }
}

#[cfg(all(test, feature = "minidom"))]
mod tests_minidom {
    use super::*;

    use alloc::{string::ToString, vec, vec::Vec};

    fn events_to_items<I: Iterator<Item = Event>>(events: I) -> Vec<Item<'static>> {
        let iter = EventToItem {
            inner: events.map(|ev| Ok(ev)),
            attributes: None,
        };
        let mut result = Vec::new();
        for item in iter {
            let item = item.unwrap();
            result.push(item);
        }
        result
    }

    #[test]
    fn event_to_item_xml_declaration() {
        let events = vec![Event::XmlDeclaration(
            EventMetrics::zero(),
            XmlVersion::V1_0,
        )];
        let items = events_to_items(events.into_iter());
        assert_eq!(items.len(), 1);
        match items[0] {
            Item::XmlDeclaration(XmlVersion::V1_0) => (),
            ref other => panic!("unexpected item in position 0: {:?}", other),
        };
    }

    #[test]
    fn event_to_item_empty_element() {
        let events = vec![
            Event::StartElement(
                EventMetrics::zero(),
                (Namespace::NONE, "elem".try_into().unwrap()),
                AttrMap::new(),
            ),
            Event::EndElement(EventMetrics::zero()),
        ];
        let items = events_to_items(events.into_iter());
        assert_eq!(items.len(), 3);
        match items[0] {
            Item::ElementHeadStart(ref ns, ref name) => {
                assert_eq!(&**ns, Namespace::none());
                assert_eq!(&**name, "elem");
            }
            ref other => panic!("unexpected item in position 0: {:?}", other),
        };
        match items[1] {
            Item::ElementHeadEnd => (),
            ref other => panic!("unexpected item in position 1: {:?}", other),
        };
        match items[2] {
            Item::ElementFoot => (),
            ref other => panic!("unexpected item in position 2: {:?}", other),
        };
    }

    #[test]
    fn event_to_item_element_with_attributes() {
        let mut attrs = AttrMap::new();
        attrs.insert(
            Namespace::NONE,
            "attr".try_into().unwrap(),
            "value".to_string(),
        );
        let events = vec![
            Event::StartElement(
                EventMetrics::zero(),
                (Namespace::NONE, "elem".try_into().unwrap()),
                attrs,
            ),
            Event::EndElement(EventMetrics::zero()),
        ];
        let items = events_to_items(events.into_iter());
        assert_eq!(items.len(), 4);
        match items[0] {
            Item::ElementHeadStart(ref ns, ref name) => {
                assert_eq!(&**ns, Namespace::none());
                assert_eq!(&**name, "elem");
            }
            ref other => panic!("unexpected item in position 0: {:?}", other),
        };
        match items[1] {
            Item::Attribute(ref ns, ref name, ref value) => {
                assert_eq!(&**ns, Namespace::none());
                assert_eq!(&**name, "attr");
                assert_eq!(&**value, "value");
            }
            ref other => panic!("unexpected item in position 1: {:?}", other),
        };
        match items[2] {
            Item::ElementHeadEnd => (),
            ref other => panic!("unexpected item in position 2: {:?}", other),
        };
        match items[3] {
            Item::ElementFoot => (),
            ref other => panic!("unexpected item in position 3: {:?}", other),
        };
    }

    #[test]
    fn event_to_item_element_with_text() {
        let events = vec![
            Event::StartElement(
                EventMetrics::zero(),
                (Namespace::NONE, "elem".try_into().unwrap()),
                AttrMap::new(),
            ),
            Event::Text(EventMetrics::zero(), "Hello World!".to_owned()),
            Event::EndElement(EventMetrics::zero()),
        ];
        let items = events_to_items(events.into_iter());
        assert_eq!(items.len(), 4);
        match items[0] {
            Item::ElementHeadStart(ref ns, ref name) => {
                assert_eq!(&**ns, Namespace::none());
                assert_eq!(&**name, "elem");
            }
            ref other => panic!("unexpected item in position 0: {:?}", other),
        };
        match items[1] {
            Item::ElementHeadEnd => (),
            ref other => panic!("unexpected item in position 1: {:?}", other),
        };
        match items[2] {
            Item::Text(ref value) => {
                assert_eq!(value, "Hello World!");
            }
            ref other => panic!("unexpected item in position 2: {:?}", other),
        };
        match items[3] {
            Item::ElementFoot => (),
            ref other => panic!("unexpected item in position 3: {:?}", other),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::{vec, vec::Vec};

    fn items_to_events<'x, I: IntoIterator<Item = Item<'x>>>(
        items: I,
    ) -> Result<Vec<Event>, crate::error::Error> {
        let iter = ItemToEvent {
            inner: items.into_iter().map(|x| Ok(x)),
            event_buffer: None,
            elem_buffer: None,
        };
        let mut result = Vec::new();
        for ev in iter {
            let ev = ev?;
            result.push(ev);
        }
        Ok(result)
    }

    #[test]
    fn item_to_event_xml_decl() {
        let items = vec![Item::XmlDeclaration(XmlVersion::V1_0)];
        let events = items_to_events(items).expect("item conversion");
        assert_eq!(events.len(), 1);
        match events[0] {
            Event::XmlDeclaration(_, XmlVersion::V1_0) => (),
            ref other => panic!("unexpected event in position 0: {:?}", other),
        };
    }

    #[test]
    fn item_to_event_simple_empty_element() {
        let items = vec![
            Item::ElementHeadStart(Namespace::NONE, Cow::Borrowed("elem".try_into().unwrap())),
            Item::ElementHeadEnd,
            Item::ElementFoot,
        ];
        let events = items_to_events(items).expect("item conversion");
        assert_eq!(events.len(), 2);
        match events[0] {
            Event::StartElement(_, (ref ns, ref name), ref attrs) => {
                assert_eq!(attrs.len(), 0);
                assert_eq!(ns, Namespace::none());
                assert_eq!(name, "elem");
            }
            ref other => panic!("unexpected event in position 0: {:?}", other),
        };
        match events[1] {
            Event::EndElement(_) => (),
            ref other => panic!("unexpected event in position 1: {:?}", other),
        };
    }

    #[test]
    fn item_to_event_short_empty_element() {
        let items = vec![
            Item::ElementHeadStart(Namespace::NONE, Cow::Borrowed("elem".try_into().unwrap())),
            Item::ElementFoot,
        ];
        let events = items_to_events(items).expect("item conversion");
        assert_eq!(events.len(), 2);
        match events[0] {
            Event::StartElement(_, (ref ns, ref name), ref attrs) => {
                assert_eq!(attrs.len(), 0);
                assert_eq!(ns, Namespace::none());
                assert_eq!(name, "elem");
            }
            ref other => panic!("unexpected event in position 0: {:?}", other),
        };
        match events[1] {
            Event::EndElement(_) => (),
            ref other => panic!("unexpected event in position 1: {:?}", other),
        };
    }

    #[test]
    fn item_to_event_element_with_text_content() {
        let items = vec![
            Item::ElementHeadStart(Namespace::NONE, Cow::Borrowed("elem".try_into().unwrap())),
            Item::ElementHeadEnd,
            Item::Text(Cow::Borrowed("Hello World!")),
            Item::ElementFoot,
        ];
        let events = items_to_events(items).expect("item conversion");
        assert_eq!(events.len(), 3);
        match events[0] {
            Event::StartElement(_, (ref ns, ref name), ref attrs) => {
                assert_eq!(attrs.len(), 0);
                assert_eq!(ns, Namespace::none());
                assert_eq!(name, "elem");
            }
            ref other => panic!("unexpected event in position 0: {:?}", other),
        };
        match events[1] {
            Event::Text(_, ref value) => {
                assert_eq!(value, "Hello World!");
            }
            ref other => panic!("unexpected event in position 1: {:?}", other),
        };
        match events[2] {
            Event::EndElement(_) => (),
            ref other => panic!("unexpected event in position 2: {:?}", other),
        };
    }

    #[test]
    fn item_to_event_element_with_attributes() {
        let items = vec![
            Item::ElementHeadStart(Namespace::NONE, Cow::Borrowed("elem".try_into().unwrap())),
            Item::Attribute(
                Namespace::NONE,
                Cow::Borrowed("attr".try_into().unwrap()),
                Cow::Borrowed("value"),
            ),
            Item::ElementHeadEnd,
            Item::ElementFoot,
        ];
        let events = items_to_events(items).expect("item conversion");
        assert_eq!(events.len(), 2);
        match events[0] {
            Event::StartElement(_, (ref ns, ref name), ref attrs) => {
                assert_eq!(ns, Namespace::none());
                assert_eq!(name, "elem");
                assert_eq!(attrs.len(), 1);
                assert_eq!(
                    attrs.get(Namespace::none(), "attr").map(|x| x.as_str()),
                    Some("value")
                );
            }
            ref other => panic!("unexpected event in position 0: {:?}", other),
        };
        match events[1] {
            Event::EndElement(_) => (),
            ref other => panic!("unexpected event in position 2: {:?}", other),
        };
    }
}
