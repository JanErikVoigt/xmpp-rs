// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    Agent, RoomNick,
    jid::{BareJid, Jid},
    message::send::RawMessageSettings,
    parsers::{message::MessageType, muc::user::MucUser},
};

#[derive(Clone, Debug)]
pub struct RoomPrivateMessageSettings<'a> {
    pub room: BareJid,
    pub recipient: RoomNick,
    pub message: &'a str,
    pub lang: Option<&'a str>,
}

impl<'a> RoomPrivateMessageSettings<'a> {
    pub fn new(room: BareJid, recipient: RoomNick, message: &'a str) -> Self {
        Self {
            room,
            recipient,
            message,
            lang: None,
        }
    }

    pub fn with_lang(mut self, lang: &'a str) -> Self {
        self.lang = Some(lang);
        self
    }
}

pub async fn send_room_private_message<'a>(
    agent: &mut Agent,
    settings: RoomPrivateMessageSettings<'a>,
) {
    let RoomPrivateMessageSettings {
        room,
        recipient,
        message,
        lang,
    } = settings;

    // TODO: check that room is in agent.joined_rooms
    let recipient: Jid = room.with_resource(recipient.as_ref()).into();
    agent
        .send_raw_message(
            RawMessageSettings::new(recipient, MessageType::Chat, message)
                .with_payload(MucUser::new())
                .with_lang_option(lang),
        )
        .await;
}
