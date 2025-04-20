// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use alloc::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
#[cfg(feature = "escape-hatch")]
use tokio::io;
use tokio::sync::RwLock;

use crate::{
    event_loop,
    jid::{BareJid, Jid},
    message, muc,
    parsers::disco::DiscoInfoResult,
    upload, Error, Event, RoomNick,
};
use tokio_xmpp::Client as TokioXmppClient;
#[cfg(feature = "escape-hatch")]
use tokio_xmpp::{stanzastream::StanzaToken, Stanza};

pub struct Agent {
    pub(crate) client: TokioXmppClient,
    pub(crate) default_nick: Arc<RwLock<RoomNick>>,
    pub(crate) lang: Arc<Vec<String>>,
    pub(crate) disco: DiscoInfoResult,
    pub(crate) node: String,
    pub(crate) uploads: Vec<(String, Jid, PathBuf)>,
    pub(crate) awaiting_disco_bookmarks_type: bool,
    // Mapping of room->nick
    pub(crate) rooms_joined: HashMap<BareJid, RoomNick>,
    pub(crate) rooms_joining: HashMap<BareJid, RoomNick>,
    pub(crate) rooms_leaving: HashMap<BareJid, RoomNick>,
}

impl Agent {
    pub fn new(
        client: TokioXmppClient,
        default_nick: RoomNick,
        lang: Vec<String>,
        disco: DiscoInfoResult,
        node: String,
    ) -> Agent {
        Agent {
            client,
            default_nick: Arc::new(RwLock::new(default_nick)),
            lang: Arc::new(lang),
            disco,
            node,
            uploads: Vec::new(),
            awaiting_disco_bookmarks_type: false,
            rooms_joined: HashMap::new(),
            rooms_joining: HashMap::new(),
            rooms_leaving: HashMap::new(),
        }
    }

    pub async fn disconnect(self) -> Result<(), Error> {
        self.client.send_end().await
    }

    #[cfg(feature = "escape-hatch")]
    pub async fn send_stanza<S: Into<Stanza>>(&mut self, st: S) -> Result<StanzaToken, io::Error> {
        self.client.send_stanza(st.into()).await
    }

    pub async fn join_room<'a>(&mut self, settings: muc::room::JoinRoomSettings<'a>) {
        muc::room::join_room(self, settings).await
    }

    /// Request to leave a chatroom.
    ///
    /// If successful, an [Event::RoomLeft] event will be produced. This method does not remove the room
    /// from bookmarks nor remove the autojoin flag. See [muc::room::leave_room] for more information.
    pub async fn leave_room<'a>(&mut self, settings: muc::room::LeaveRoomSettings<'a>) {
        muc::room::leave_room(self, settings).await
    }

    pub async fn send_raw_message<'a>(&mut self, settings: message::send::RawMessageSettings<'a>) {
        message::send::send_raw_message(self, settings).await
    }

    pub async fn send_message<'a>(&mut self, settings: message::send::MessageSettings<'a>) {
        message::send::send_message(self, settings).await
    }

    pub async fn send_room_message<'a>(&mut self, settings: muc::room::RoomMessageSettings<'a>) {
        muc::room::send_room_message(self, settings).await
    }

    pub async fn send_room_private_message<'a>(
        &mut self,
        settings: muc::private_message::RoomPrivateMessageSettings<'a>,
    ) {
        muc::private_message::send_room_private_message(self, settings).await
    }

    /// Wait for new events, or Error::Disconnected when connection is closed and will not reconnect.
    pub async fn wait_for_events(&mut self) -> Vec<Event> {
        event_loop::wait_for_events(self).await
    }

    pub async fn upload_file_with(&mut self, service: &str, path: &Path) {
        upload::send::upload_file_with(self, service, path).await
    }

    /// Get the bound jid of the client.
    ///
    /// If the client is not connected, this will be None.
    pub fn bound_jid(&self) -> Option<&Jid> {
        self.client.bound_jid()
    }
}
