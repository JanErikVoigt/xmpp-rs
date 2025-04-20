// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    jid::{BareJid, ResourceRef},
    message::send::RawMessageSettings,
    parsers::{
        message::MessageType,
        muc::Muc,
        presence::{Presence, Type as PresenceType},
    },
    Agent, RoomNick,
};

#[derive(Clone, Debug)]
pub struct JoinRoomSettings<'a> {
    pub room: BareJid,
    pub nick: Option<RoomNick>,
    pub password: Option<String>,
    pub status: Option<(&'a str, &'a str)>,
}

impl<'a> JoinRoomSettings<'a> {
    pub fn new(room: BareJid) -> Self {
        Self {
            room,
            nick: None,
            password: None,
            status: None,
        }
    }

    pub fn with_nick(mut self, nick: impl AsRef<ResourceRef>) -> Self {
        self.nick = Some(RoomNick::from_resource_ref(nick.as_ref()));
        self
    }

    pub fn with_password(mut self, password: impl AsRef<str>) -> Self {
        self.password = Some(password.as_ref().into());
        self
    }

    pub fn with_status(mut self, lang: &'a str, content: &'a str) -> Self {
        self.status = Some((lang, content));
        self
    }
}

/// TODO: this method should add bookmark and ensure autojoin is true
pub async fn join_room<'a>(agent: &mut Agent, settings: JoinRoomSettings<'a>) {
    let JoinRoomSettings {
        room,
        nick,
        password,
        status,
    } = settings;

    if agent.rooms_joining.contains_key(&room) {
        // We are already joining
        warn!("Requesting to join again room {room} which is already joining...");
        return;
    }

    if agent.rooms_joined.contains_key(&room) {
        // We are already joined, cannot join
        warn!("Requesting to join room {room} which is already joined...");
        return;
    }

    let mut muc = Muc::new();
    if let Some(password) = password {
        muc = muc.with_password(password);
    }

    let nick = if let Some(nick) = nick {
        nick
    } else {
        agent.default_nick.read().await.clone()
    };

    let room_jid = room.with_resource(&nick);
    let mut presence = Presence::new(PresenceType::None).with_to(room_jid);
    presence.add_payload(muc);

    let (lang, status) = status.unwrap_or(("", ""));
    presence.set_status(String::from(lang), String::from(status));

    let _ = agent.client.send_stanza(presence.into()).await;

    agent.rooms_joining.insert(room, nick);
}

#[derive(Clone, Debug)]
pub struct LeaveRoomSettings<'a> {
    pub room: BareJid,
    pub status: Option<(&'a str, &'a str)>,
}

impl<'a> LeaveRoomSettings<'a> {
    pub fn new(room: BareJid) -> Self {
        Self { room, status: None }
    }

    pub fn with_status(mut self, lang: &'a str, content: &'a str) -> Self {
        self.status = Some((lang, content));
        self
    }
}

/// Send a "leave room" request to the server (specifically, an "unavailable" presence stanza).
///
/// The returned future will resolve when the request has been sent,
/// not when the room has actually been left.
///
/// If successful, a `RoomLeft` event should be received later as a confirmation. See [XEP-0045](https://xmpp.org/extensions/xep-0045.html#exit).
///
/// TODO: this method should set autojoin false on bookmark
pub async fn leave_room<'a>(agent: &mut Agent, settings: LeaveRoomSettings<'a>) {
    let LeaveRoomSettings { room, status } = settings;

    if agent.rooms_leaving.contains_key(&room) {
        // We are already leaving
        warn!("Requesting to leave again room {room} which is already leaving...");
        return;
    }

    if !agent.rooms_joined.contains_key(&room) {
        // We are not joined, cannot leave
        warn!("Requesting to leave room {room} which is not joined...");
        return;
    }

    // Get currently-used nickname
    let nickname = agent.rooms_joined.get(&room).unwrap();

    // XEP-0045 specifies that, to leave a room, the client must send a presence stanza
    // with type="unavailable".
    let mut presence =
        Presence::new(PresenceType::Unavailable).with_to(room.with_resource(&nickname));

    // Optionally, the client may include a status message in the presence stanza.
    // TODO: Should this be optional? The XEP says "MAY", but the method signature requires the arguments.
    // XEP-0045: "The occupant MAY include normal <status/> information in the unavailable presence stanzas"
    if let Some((lang, content)) = status {
        presence.set_status(lang, content);
    }

    // Send the presence stanza.
    if let Err(e) = agent.client.send_stanza(presence.into()).await {
        // Report any errors to the log.
        error!("Failed to send leave room presence: {}", e);
    }

    agent.rooms_leaving.insert(room, nickname.clone());
}

#[derive(Clone, Debug)]
pub struct RoomMessageSettings<'a> {
    pub room: BareJid,
    pub message: &'a str,
    pub lang: Option<&'a str>,
}

impl<'a> RoomMessageSettings<'a> {
    pub fn new(room: BareJid, message: &'a str) -> Self {
        Self {
            room,
            message,
            lang: None,
        }
    }

    pub fn with_lang(mut self, lang: &'a str) -> Self {
        self.lang = Some(lang);
        self
    }
}

pub async fn send_room_message<'a>(agent: &mut Agent, settings: RoomMessageSettings<'a>) {
    let RoomMessageSettings {
        room,
        message,
        lang,
    } = settings;

    // TODO: check that room is in agent.joined_rooms
    agent
        .send_raw_message(
            RawMessageSettings::new(room.into(), MessageType::Groupchat, message)
                .with_lang_option(lang),
        )
        .await;
}
