// Copyright (c) 2017 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use alloc::borrow::Cow;
use core::str::FromStr;

use xso::{error::Error, AsXmlText, FromXmlText, TextCodec};

use chrono::{DateTime as ChronoDateTime, FixedOffset, Utc};
use minidom::{IntoAttributeValue, Node};

/// Text codec for
/// [XEP-0082](https://xmpp.org/extensions/xep-0082.html)-compliant formatting
/// of dates and times.
pub struct Xep0082;

impl TextCodec<ChronoDateTime<FixedOffset>> for Xep0082 {
    fn decode(&self, s: String) -> Result<ChronoDateTime<FixedOffset>, Error> {
        Ok(ChronoDateTime::parse_from_rfc3339(&s).map_err(Error::text_parse_error)?)
    }

    fn encode<'x>(
        &self,
        value: &'x ChronoDateTime<FixedOffset>,
    ) -> Result<Option<Cow<'x, str>>, Error> {
        if value.offset().utc_minus_local() == 0 {
            Ok(Some(Cow::Owned(
                value.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            )))
        } else {
            Ok(Some(Cow::Owned(value.to_rfc3339())))
        }
    }
}

impl TextCodec<ChronoDateTime<Utc>> for Xep0082 {
    fn decode(&self, s: String) -> Result<ChronoDateTime<Utc>, Error> {
        Ok(ChronoDateTime::<FixedOffset>::parse_from_rfc3339(&s)
            .map_err(Error::text_parse_error)?
            .into())
    }

    fn encode<'x>(&self, value: &'x ChronoDateTime<Utc>) -> Result<Option<Cow<'x, str>>, Error> {
        Ok(Some(Cow::Owned(
            value.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        )))
    }
}

impl<T> TextCodec<Option<T>> for Xep0082
where
    Xep0082: TextCodec<T>,
{
    fn decode(&self, s: String) -> Result<Option<T>, Error> {
        Ok(Some(self.decode(s)?))
    }

    fn encode<'x>(&self, value: &'x Option<T>) -> Result<Option<Cow<'x, str>>, Error> {
        value
            .as_ref()
            .and_then(|x| self.encode(x).transpose())
            .transpose()
    }
}

/// Implements the DateTime profile of XEP-0082, which represents a
/// non-recurring moment in time, with an accuracy of seconds or fraction of
/// seconds, and includes a timezone.
#[derive(Debug, Clone, PartialEq)]
pub struct DateTime(pub ChronoDateTime<FixedOffset>);

impl DateTime {
    /// Retrieves the associated timezone.
    pub fn timezone(&self) -> FixedOffset {
        self.0.timezone()
    }

    /// Returns a new `DateTime` with a different timezone.
    pub fn with_timezone(&self, tz: FixedOffset) -> DateTime {
        DateTime(self.0.with_timezone(&tz))
    }

    /// Formats this `DateTime` with the specified format string.
    pub fn format(&self, fmt: &str) -> String {
        format!("{}", self.0.format(fmt))
    }
}

impl FromStr for DateTime {
    type Err = chrono::ParseError;

    fn from_str(s: &str) -> Result<DateTime, Self::Err> {
        Ok(DateTime(ChronoDateTime::parse_from_rfc3339(s)?))
    }
}

impl FromXmlText for DateTime {
    fn from_xml_text(s: String) -> Result<Self, Error> {
        s.parse().map_err(Error::text_parse_error)
    }
}

impl AsXmlText for DateTime {
    fn as_xml_text(&self) -> Result<Cow<'_, str>, Error> {
        Ok(Cow::Owned(self.0.to_rfc3339()))
    }
}

impl IntoAttributeValue for DateTime {
    fn into_attribute_value(self) -> Option<String> {
        Some(self.0.to_rfc3339())
    }
}

impl From<DateTime> for Node {
    fn from(date: DateTime) -> Node {
        Node::Text(date.0.to_rfc3339())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    // DateTime’s size doesn’t depend on the architecture.
    #[test]
    fn test_size() {
        assert_size!(DateTime, 16);
    }

    #[test]
    fn test_simple() {
        let date: DateTime = "2002-09-10T23:08:25Z".parse().unwrap();
        assert_eq!(date.0.year(), 2002);
        assert_eq!(date.0.month(), 9);
        assert_eq!(date.0.day(), 10);
        assert_eq!(date.0.hour(), 23);
        assert_eq!(date.0.minute(), 08);
        assert_eq!(date.0.second(), 25);
        assert_eq!(date.0.nanosecond(), 0);
        assert_eq!(date.0.timezone(), FixedOffset::east_opt(0).unwrap());
    }

    #[test]
    fn test_invalid_date() {
        // There is no thirteenth month.
        let error = DateTime::from_str("2017-13-01T12:23:34Z").unwrap_err();
        assert_eq!(error.to_string(), "input is out of range");

        // Timezone ≥24:00 aren’t allowed.
        let error = DateTime::from_str("2017-05-27T12:11:02+25:00").unwrap_err();
        assert_eq!(error.to_string(), "input is out of range");

        // Timezone without the : separator aren’t allowed.
        let error = DateTime::from_str("2017-05-27T12:11:02+0100").unwrap_err();
        assert_eq!(error.to_string(), "input contains invalid characters");

        // No seconds, error message could be improved.
        let error = DateTime::from_str("2017-05-27T12:11+01:00").unwrap_err();
        assert_eq!(error.to_string(), "input contains invalid characters");

        // TODO: maybe we’ll want to support this one, as per XEP-0082 §4.
        let error = DateTime::from_str("20170527T12:11:02+01:00").unwrap_err();
        assert_eq!(error.to_string(), "input contains invalid characters");

        // No timezone.
        let error = DateTime::from_str("2017-05-27T12:11:02").unwrap_err();
        assert_eq!(error.to_string(), "premature end of input");
    }

    #[test]
    fn test_serialise() {
        let date =
            DateTime(ChronoDateTime::parse_from_rfc3339("2017-05-21T20:19:55+01:00").unwrap());
        let attr = date.into_attribute_value();
        assert_eq!(attr, Some(String::from("2017-05-21T20:19:55+01:00")));
    }
}
