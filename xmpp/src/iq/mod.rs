// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use tokio_xmpp::parsers::iq::{Iq, IqHeader, IqPayload};

use crate::{Agent, Event};

pub mod get;
pub mod result;
pub mod set;

pub async fn handle_iq(agent: &mut Agent, iq: Iq) -> Vec<Event> {
    let mut events = vec![];
    let (IqHeader { from, to, id }, data) = iq.split();
    let from = from.unwrap_or_else(|| agent.client.bound_jid().unwrap().to_bare().into());
    if let IqPayload::Get(payload) = data {
        get::handle_iq_get(agent, &mut events, from, to, id, payload).await;
    } else if let IqPayload::Result(Some(payload)) = data {
        result::handle_iq_result(agent, &mut events, from, to, id, payload).await;
    } else if let IqPayload::Set(payload) = data {
        set::handle_iq_set(agent, &mut events, from, to, id, payload).await;
    }
    events
}
