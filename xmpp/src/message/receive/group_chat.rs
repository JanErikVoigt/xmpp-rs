// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    Agent, Event, RoomNick,
    delay::StanzaTimeInfo,
    jid::Jid,
    parsers::{message::Message, message_correct::Replace},
};

pub async fn handle_message_group_chat(
    agent: &mut Agent,
    events: &mut Vec<Event>,
    from: Jid,
    message: &mut Message,
    time_info: StanzaTimeInfo,
) {
    let config = agent.config.read().await;
    let langs: Vec<&str> = config.lang.iter().map(String::as_str).collect();
    let mut found_subject = false;

    if let Some((_lang, subject)) = message.get_best_subject(langs.clone()) {
        events.push(Event::RoomSubject(
            from.to_bare(),
            from.resource().map(RoomNick::from_resource_ref),
            subject.clone(),
            time_info.clone(),
        ));
        found_subject = true;
    }

    let Some((_lang, body)) = message.get_best_body_cloned(langs) else {
        if !found_subject {
            debug!(
                "Received groupchat message without body/subject:\n{:#?}",
                message
            );
        }
        return;
    };

    let correction = message.extract_payload::<Replace>().unwrap_or_else(|e| {
        warn!("Failed to parse <replace> payload: {e}");
        None
    });

    // Now we have a groupchat message... which can be:
    //
    // - a normal MUC message from a user in a room
    // - a MUC message correction from a user in a room
    // - a service message from a MUC channel (barejid)
    //
    // In theory we can have service message correction but nope nope nope

    if let Some(resource) = from.resource() {
        // User message/correction

        let event = if let Some(correction) = correction {
            Event::RoomMessageCorrection(
                correction.id,
                from.to_bare(),
                RoomNick::from_resource_ref(resource),
                body.clone(),
                time_info,
            )
        } else {
            Event::RoomMessage(
                message.id.clone(),
                from.to_bare(),
                RoomNick::from_resource_ref(resource),
                body.clone(),
                time_info,
            )
        };
        events.push(event);
    } else {
        // Service message
        if correction.is_some() {
            warn!("Found correction in service message:\n{:#?}", message);
        } else {
            let event = Event::ServiceMessage(message.id.clone(), from.to_bare(), body, time_info);
            events.push(event);
        }
    }
}
