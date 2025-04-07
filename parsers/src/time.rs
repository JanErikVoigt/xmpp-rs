// Copyright (c) 2019 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use alloc::borrow::Cow;

use xso::{AsXml, FromXml};

use crate::date::Xep0082;
use crate::iq::{IqGetPayload, IqResultPayload};
use crate::ns;
use chrono::{DateTime, FixedOffset, Utc};
use core::str::FromStr;
use xso::{error::Error, text::TextCodec};

struct ColonSeparatedOffset;

impl TextCodec<FixedOffset> for ColonSeparatedOffset {
    fn decode(&self, s: String) -> Result<FixedOffset, Error> {
        FixedOffset::from_str(&s).map_err(Error::text_parse_error)
    }

    fn encode<'x>(&self, value: &'x FixedOffset) -> Result<Option<Cow<'x, str>>, Error> {
        let offset = value.local_minus_utc();
        let nminutes = offset / 60;
        let nseconds = offset % 60;
        let nhours = nminutes / 60;
        let nminutes = nminutes % 60;
        if nseconds == 0 {
            Ok(Some(Cow::Owned(format!("{:+03}:{:02}", nhours, nminutes))))
        } else {
            Ok(Some(Cow::Owned(format!(
                "{:+03}:{:02}:{:02}",
                nhours, nminutes, nseconds
            ))))
        }
    }
}

/// An entity time query.
#[derive(FromXml, AsXml, PartialEq, Debug, Clone)]
#[xml(namespace = ns::TIME, name = "time")]
pub struct TimeQuery;

impl IqGetPayload for TimeQuery {}

/// An entity time result, containing an unique DateTime.
#[derive(Debug, Clone, Copy, FromXml, AsXml)]
#[xml(namespace = ns::TIME, name = "time")]
pub struct TimeResult {
    /// The UTC offset
    #[xml(extract(name = "tzo", fields(text(codec = ColonSeparatedOffset))))]
    pub tz_offset: FixedOffset,

    /// The UTC timestamp
    #[xml(extract(name = "utc", fields(text(codec = Xep0082))))]
    pub utc: DateTime<Utc>,
}

impl IqResultPayload for TimeResult {}

impl From<TimeResult> for DateTime<FixedOffset> {
    fn from(time: TimeResult) -> Self {
        time.utc.with_timezone(&time.tz_offset)
    }
}

impl From<TimeResult> for DateTime<Utc> {
    fn from(time: TimeResult) -> Self {
        time.utc
    }
}

impl From<DateTime<FixedOffset>> for TimeResult {
    fn from(dt: DateTime<FixedOffset>) -> Self {
        let tz_offset = *dt.offset();
        let utc = dt.with_timezone(&Utc);
        TimeResult { tz_offset, utc }
    }
}

impl From<DateTime<Utc>> for TimeResult {
    fn from(dt: DateTime<Utc>) -> Self {
        TimeResult {
            tz_offset: FixedOffset::east_opt(0).unwrap(),
            utc: dt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use minidom::Element;

    // DateTime’s size doesn’t depend on the architecture.
    #[test]
    fn test_size() {
        assert_size!(TimeQuery, 0);
        assert_size!(TimeResult, 16);
    }

    #[test]
    fn parse_response() {
        let elem: Element =
            "<time xmlns='urn:xmpp:time'><tzo>-06:00</tzo><utc>2006-12-19T17:58:35Z</utc></time>"
                .parse()
                .unwrap();
        let elem1 = elem.clone();
        let time = TimeResult::try_from(elem).unwrap();
        let dt = DateTime::<FixedOffset>::from(time);
        assert_eq!(dt.timezone(), FixedOffset::west_opt(6 * 3600).unwrap());
        assert_eq!(
            dt,
            DateTime::<FixedOffset>::from_str("2006-12-19T12:58:35-05:00").unwrap()
        );
        let elem2 = Element::from(time);
        assert_eq!(elem1, elem2);
    }
}
