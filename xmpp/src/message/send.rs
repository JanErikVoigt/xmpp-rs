// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    jid::{BareJid, Jid},
    minidom::Element,
    parsers::message::{Message, MessagePayload, MessageType},
};

use crate::Agent;

#[derive(Clone, Debug)]
pub struct RawMessageSettings<'a> {
    pub recipient: Jid,
    pub message_type: MessageType,
    pub message: &'a str,
    pub lang: Option<&'a str>,
    pub payloads: Vec<Element>,
}

impl<'a> RawMessageSettings<'a> {
    pub fn new(recipient: Jid, message_type: MessageType, message: &'a str) -> Self {
        Self {
            recipient,
            message_type,
            message,
            lang: None,
            payloads: Vec::new(),
        }
    }

    pub fn with_lang(mut self, lang: &'a str) -> Self {
        self.lang = Some(lang);
        self
    }

    pub fn with_lang_option(mut self, lang: Option<&'a str>) -> Self {
        self.lang = lang;
        self
    }

    pub fn with_payload(mut self, payload: impl MessagePayload) -> Self {
        self.payloads.push(payload.into());
        self
    }
}

pub async fn send_raw_message<'a>(agent: &mut Agent, settings: RawMessageSettings<'a>) {
    let RawMessageSettings {
        recipient,
        message_type,
        message,
        lang,
        payloads,
    } = settings;

    let mut stanza = Message::new(Some(recipient));

    for payload in payloads {
        stanza.payloads.push(payload);
    }

    stanza.type_ = message_type;
    stanza
        .bodies
        .insert(lang.unwrap_or("").into(), String::from(message));
    agent.client.send_stanza(stanza.into()).await.unwrap();
}

#[derive(Clone, Debug)]
pub struct MessageSettings<'a> {
    pub recipient: BareJid,
    pub message: &'a str,
    pub lang: Option<&'a str>,
}

impl<'a> MessageSettings<'a> {
    pub fn new(recipient: BareJid, message: &'a str) -> Self {
        Self {
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

pub async fn send_message<'a>(agent: &mut Agent, settings: MessageSettings<'a>) {
    let MessageSettings {
        recipient,
        message,
        lang,
    } = settings;

    // TODO: check that recipient is not in agent.joined_rooms
    agent
        .send_raw_message(
            RawMessageSettings::new(recipient.into(), MessageType::Chat, message)
                .with_lang_option(lang),
        )
        .await;
}
