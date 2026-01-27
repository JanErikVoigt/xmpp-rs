// Copyright (c) 2019 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use core::ops::ControlFlow;
use core::{pin::Pin, task::Context};
use futures::{ready, task::Poll, Stream};

use crate::{
    client::Client,
    stanzastream::{Event as StanzaStreamEvent, StreamEvent},
    Event, Stanza,
};

/// Incoming XMPP events
///
/// In an `async fn` you may want to use this with `use
/// futures::stream::StreamExt;`
impl Stream for Client {
    type Item = Event;

    /// Low-level read on the XMPP stream, allowing the underlying
    /// machinery to:
    ///
    /// * connect,
    /// * starttls,
    /// * authenticate,
    /// * bind a session, and finally
    /// * receive stanzas
    ///
    /// ...for your client
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        loop {
            return Poll::Ready(match ready!(Pin::new(&mut self.stream).poll_next(cx)) {
                None => None,
                Some(StanzaStreamEvent::Stanza(st)) => match st {
                    Stanza::Iq(iq) => match self.iq_response_tracker.handle_iq(iq) {
                        ControlFlow::Break(()) => continue,
                        ControlFlow::Continue(iq) => Some(Event::Stanza(Stanza::Iq(iq))),
                    },
                    other => Some(Event::Stanza(other)),
                },
                Some(StanzaStreamEvent::Stream(StreamEvent::Reset {
                    bound_jid,
                    features,
                })) => {
                    self.features = Some(features);
                    self.bound_jid = Some(bound_jid.clone());

                    self.iq_response_tracker
                        .set_account_jid(bound_jid.to_bare());

                    Some(Event::Online {
                        bound_jid,
                        resumed: false,
                    })
                }
                Some(StanzaStreamEvent::Stream(StreamEvent::Resumed)) => Some(Event::Online {
                    bound_jid: self.bound_jid.as_ref().unwrap().clone(),
                    resumed: true,
                }),
                Some(StanzaStreamEvent::Stream(StreamEvent::Suspended)) => continue,
            });
        }
    }
}
