// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use futures::StreamExt;
use tokio_xmpp::{
    Event as TokioXmppEvent, Stanza,
    parsers::{disco::DiscoInfoQuery, iq::Iq, roster::Roster},
};

use crate::{Agent, Event, iq, message, presence};

/// Wait for new events, or Error::Disconnected when stream is closed and will not reconnect.
pub async fn wait_for_events(agent: &mut Agent) -> Vec<Event> {
    if let Some(event) = agent.client.next().await {
        let mut events = Vec::new();

        match event {
            TokioXmppEvent::Online { resumed: false, .. } => {
                let presence =
                    presence::send::make_initial_presence(&agent.disco, &agent.node).into();
                let _ = agent.client.send_stanza(presence).await;
                events.push(Event::Online);
                // TODO: only send this when the ContactList feature is enabled.
                let iq = Iq::from_get(
                    "roster",
                    Roster {
                        ver: None,
                        items: vec![],
                    },
                )
                .into();
                let _ = agent.client.send_stanza(iq).await;

                // Query account disco to know what bookmarks spec is used
                let iq = Iq::from_get("disco-account", DiscoInfoQuery { node: None }).into();
                let _ = agent.client.send_stanza(iq).await;
                agent.awaiting_disco_bookmarks_type = true;
            }
            TokioXmppEvent::Online { resumed: true, .. } => {}
            TokioXmppEvent::Disconnected(e) => {
                events.push(Event::Disconnected(e));
            }
            TokioXmppEvent::Stanza(Stanza::Iq(iq)) => {
                #[cfg(feature = "escape-hatch")]
                events.push(Event::Iq(iq.clone()));

                let new_events = iq::handle_iq(agent, iq).await;
                events.extend(new_events);
            }
            TokioXmppEvent::Stanza(Stanza::Message(message)) => {
                #[cfg(feature = "escape-hatch")]
                events.push(Event::Message(message.clone()));

                let new_events = message::receive::handle_message(agent, message).await;
                events.extend(new_events);
            }
            TokioXmppEvent::Stanza(Stanza::Presence(presence)) => {
                #[cfg(feature = "escape-hatch")]
                events.push(Event::Presence(presence.clone()));

                let new_events = presence::receive::handle_presence(agent, presence).await;
                events.extend(new_events);
            }
        }

        events
    } else {
        // Stream was closed and not opening again because TokioXmppClient reconnect is false
        // However we set reconnect true in agent builder so this should never happen and indicates
        // logic error in tokio_xmpp::AsyncClient::poll_next
        panic!("xmpp::Agent should never receive None event (stream closed, no reconnect)");
    }
}
