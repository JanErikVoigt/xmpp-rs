// Copyright (c) 2019 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![deny(bare_trait_objects)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

extern crate alloc;

pub use tokio_xmpp;
pub use tokio_xmpp::jid;
pub use tokio_xmpp::minidom;
pub use tokio_xmpp::parsers;

#[macro_use]
extern crate log;

use core::fmt;
use jid::{ResourcePart, ResourceRef};
use parsers::message::Id as MessageId;

pub mod agent;
pub mod builder;
pub mod delay;
pub mod disco;
pub mod event;
pub mod event_loop;
pub mod feature;
pub mod iq;
pub mod message;
pub mod muc;
pub mod presence;
pub mod pubsub;
pub mod upload;

pub use agent::Agent;
pub use builder::{ClientBuilder, ClientType};
pub use event::Event;
pub use feature::ClientFeature;

pub type Error = tokio_xmpp::Error;

/// Nickname for a person in a chatroom.
///
/// This nickname is not associated with a specific chatroom, or with a certain
/// user account.
///
// TODO: Introduce RoomMember and track by occupant-id
#[derive(Clone, Debug)]
pub struct RoomNick(ResourcePart);

impl RoomNick {
    pub fn new(nick: ResourcePart) -> Self {
        Self(nick)
    }

    pub fn from_resource_ref(nick: &ResourceRef) -> Self {
        Self(nick.to_owned())
    }
}

impl AsRef<ResourceRef> for RoomNick {
    fn as_ref(&self) -> &ResourceRef {
        self.0.as_ref()
    }
}

impl From<RoomNick> for ResourcePart {
    fn from(room_nick: RoomNick) -> Self {
        room_nick.0
    }
}

impl fmt::Display for RoomNick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::str::FromStr for RoomNick {
    type Err = crate::jid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(ResourcePart::new(s)?.into()))
    }
}

impl core::ops::Deref for RoomNick {
    type Target = ResourcePart;

    fn deref(&self) -> &ResourcePart {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reexports() {
        #[allow(unused_imports)]
        use crate::jid;
        #[allow(unused_imports)]
        use crate::minidom;
        #[allow(unused_imports)]
        use crate::parsers;
        #[allow(unused_imports)]
        use crate::tokio_xmpp;
    }
}

// The test below is dysfunctional since we have moved to StanzaStream. The
// StanzaStream will attempt to connect to foo@bar indefinitely.
// Keeping it here as inspiration for future integration tests.
/*
#[cfg(all(test, any(feature = "starttls-rust", feature = "starttls-native")))]
mod tests {
    use super::jid::{BareJid, ResourcePart};
    use super::{ClientBuilder, ClientFeature, ClientType, Event};
    use std::str::FromStr;
    use tokio_xmpp::Client as TokioXmppClient;

    #[tokio::test]
    async fn test_simple() {
        let jid = BareJid::from_str("foo@bar").unwrap();
        let nick = RoomNick::from_str("bot").unwrap();

        let client = TokioXmppClient::new(jid.clone(), "meh");

        // Client instance
        let client_builder = ClientBuilder::new(jid, "meh")
            .set_client(ClientType::Bot, "xmpp-rs")
            .set_website("https://xmpp.rs")
            .set_default_nick(nick)
            .enable_feature(ClientFeature::ContactList);

        #[cfg(feature = "avatars")]
        let client_builder = client_builder.enable_feature(ClientFeature::Avatars);

        let mut agent = client_builder.build_impl(client);

        loop {
            let events = agent.wait_for_events().await;
            assert!(match events[0] {
                Event::Disconnected(_) => true,
                _ => false,
            });
            assert_eq!(events.len(), 1);
            break;
        }
    }
}
*/
