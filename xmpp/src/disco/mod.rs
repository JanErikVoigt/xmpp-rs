// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use tokio_xmpp::{
    jid::Jid,
    parsers::{
        bookmarks,
        disco::DiscoInfoResult,
        iq::Iq,
        ns,
        private::Query as PrivateXMLQuery,
        pubsub::pubsub::{Items, PubSub},
    },
};

use crate::Agent;

pub async fn handle_disco_info_result(agent: &mut Agent, disco: DiscoInfoResult, from: Jid) {
    // Safe unwrap because no DISCO is received when we are not online
    if from == agent.client.bound_jid().unwrap().to_bare() && agent.awaiting_disco_bookmarks_type {
        info!("Received disco info about bookmarks type");
        // Trigger bookmarks query
        // TODO: only send this when the JoinRooms feature is enabled.
        agent.awaiting_disco_bookmarks_type = false;
        let mut perform_bookmarks2 = false;
        for feature in disco.features {
            if feature.var == "urn:xmpp:bookmarks:1#compat" {
                perform_bookmarks2 = true;
            }
        }

        if perform_bookmarks2 {
            // XEP-0402 bookmarks (modern)
            let iq = Iq::from_get("bookmarks", PubSub::Items(Items::new(ns::BOOKMARKS2))).into();
            let _ = agent.client.send_stanza(iq).await;
        } else {
            // XEP-0048 v1.0 bookmarks (legacy)
            let iq = Iq::from_get(
                "bookmarks-legacy",
                PrivateXMLQuery {
                    storage: bookmarks::Storage::new(),
                },
            )
            .into();
            let _ = agent.client.send_stanza(iq).await;
        }
    } else {
        info!("Ignored disco#info response from {}", from);
    }
}
