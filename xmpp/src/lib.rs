// Copyright (c) 2019 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//! # Cargo features
//!
//! ## TLS backends
//!
//! - `aws_lc_rs` (default) enables rustls with the `aws_lc_rs` backend.
//! - `ring` enables rustls with the `ring` backend`.
//! - `rustls-any-backend` enables rustls, but without enabling a backend. It
//!   is the application's responsibility to ensure that a backend is enabled
//!   and installed.
//! - `ktls` enables the use of ktls.
//!   **Important:** Currently, connections will fail if the `tls` kernel
//!   module is not available. There is no fallback to non-ktls connections!
//! - `native-tls` enables the system-native TLS library (commonly
//!   libssl/OpenSSL).
//!
//! **Note:** It is not allowed to mix rustls-based TLS backends with
//! `tls-native`. Attempting to do so will result in a compilation error.
//!
//! **Note:** The `ktls` feature requires at least one `rustls` backend to be
//! enabled (`aws_lc_rs` or `ring`).
//!
//! **Note:** When enabling not exactly one rustls backend, it is the
//! application's responsibility to make sure that a default crypto provider is
//! installed in `rustls`. Otherwise, all TLS connections will fail.
//!
//! ## Certificate validation
//!
//! When using `native-tls`, the system's native certificate store is used.
//! Otherwise, you need to pick one of the following to ensure that TLS
//! connections will succeed:
//!
//! - `rustls-native-certs` (default): Uses [rustls-native-certs](https://crates.io/crates/rustls-native-certs).
//! - `webpki-roots`: Uses [webpki-roots](https://crates.io/crates/webpki-roots).
//!
//! ## Other features
//!
//! - `starttls` (default): Enables support for `<starttls/>`. Required as per
//!   RFC 6120.
//! - `avatars` (default): Enables support for avatars.
//! - `serde`: Enable the `serde` feature in `tokio-xmpp`.
//! - `escape-hatch`: Allow access to low-level API to bypass shortcomings of the current API.

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
