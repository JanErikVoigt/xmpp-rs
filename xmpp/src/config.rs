// Copyright (c) 2025 Crate authors
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::RoomNick;
use core::str::FromStr;

/// [Disco](https://xmpp.org/registrar/disco-categories.html#client) identity type
#[derive(Debug)]
pub enum ClientType {
    Bot,
    Pc,
}

impl Default for ClientType {
    fn default() -> Self {
        ClientType::Bot
    }
}

impl ToString for ClientType {
    fn to_string(&self) -> String {
        String::from(match self {
            ClientType::Bot => "bot",
            ClientType::Pc => "pc",
        })
    }
}

/// Store Agent configuration. Differs from state which is generated at runtime
#[derive(Debug)]
pub struct Config {
    /// Synchronize bookmarks based on autojoin flag.
    /// The client will join and leave based on the value of the `autojoin` flag on the (pubsub)
    /// bookmark item.
    /// If this `bookmarks_autojoin` attribute is set to false, `autojoin` set to false won't make
    /// the client leave a room, neither will the removal of a bookmark item. This will only happen
    /// after the client is restarted, as these items won't be automatically joined anymore.
    /// <https://xmpp.org/extensions/xep-0402.html#notification>
    pub bookmarks_autojoin: bool,

    /// Nickname to use if no other is specified.
    pub default_nick: RoomNick,

    /// Client “Disco” identity.
    pub disco: (ClientType, String),

    /// Default language conveyed in stanza. Mostly useful for messages content.
    pub lang: Vec<String>,

    /// Project website advertized in client capabilities.
    pub website: String,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    fn new() -> Self {
        Config {
            bookmarks_autojoin: true,
            default_nick: RoomNick::from_str("xmpp-rs").unwrap(),
            disco: (ClientType::default(), String::from("tokio-xmpp")),
            lang: vec![String::from("en")],
            website: String::from("https://gitlab.com/xmpp-rs/tokio-xmpp"),
        }
    }
}
