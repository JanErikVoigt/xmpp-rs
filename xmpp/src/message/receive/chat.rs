// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use tokio_xmpp::{
    jid::Jid,
    parsers::{confirm::Confirm, message::Message, message_correct::Replace, muc::user::MucUser},
};

use crate::{delay::StanzaTimeInfo, Agent, Event, RoomNick};

pub async fn handle_message_chat(
    agent: &mut Agent,
    events: &mut Vec<Event>,
    from: Jid,
    message: &mut Message,
    time_info: StanzaTimeInfo,
) {
    let langs: Vec<&str> = agent.lang.iter().map(String::as_str).collect();

    let confirm = message.extract_valid_payload::<Confirm>();
    let is_muc_pm = message.extract_valid_payload::<MucUser>().is_some();

    // First process events that do not require the message body
    if !is_muc_pm && confirm.is_some() {
        events.push(Event::AuthConfirm(
            from.to_bare(),
            confirm.unwrap(),
            time_info,
        ));
        return;
    }

    // For other events, a message body is required, so stop here if there isn't one
    let Some((_lang, body)) = message.get_best_body_cloned(langs) else {
        debug!("Received normal/chat message without body:\n{:#?}", message);
        return;
    };

    let correction = message.extract_valid_payload::<Replace>();

    if is_muc_pm {
        if from.resource().is_none() {
            warn!("Received malformed MessageType::Chat in muc#user namespace from a bare JID:\n{:#?}", message);
        } else {
            let full_from = from.clone().try_into_full().unwrap();

            let event = if let Some(correction) = correction {
                Event::RoomPrivateMessageCorrection(
                    correction.id,
                    full_from.to_bare(),
                    RoomNick::from_resource_ref(full_from.resource()),
                    body.clone(),
                    time_info,
                )
            } else {
                Event::RoomPrivateMessage(
                    message.id.clone(),
                    from.to_bare(),
                    RoomNick::from_resource_ref(full_from.resource()),
                    body.clone(),
                    time_info,
                )
            };
            events.push(event);
        }
    } else {
        let event = if let Some(correction) = correction {
            // TODO: Check that correction is valid (only for last N minutes or last N messages)
            Event::ChatMessageCorrection(correction.id, from.to_bare(), body.clone(), time_info)
        } else {
            Event::ChatMessage(message.id.clone(), from.to_bare(), body, time_info)
        };
        events.push(event);
    }
}

pub async fn handle_message_error(
    _agent: &mut Agent,
    events: &mut Vec<Event>,
    from: Jid,
    message: &mut Message,
    time_info: StanzaTimeInfo,
) {
    let confirm = message.extract_valid_payload::<Confirm>();
    if let Some(confirm) = confirm {
        events.push(Event::AuthReject(from.to_bare(), confirm, time_info));
    }
}
