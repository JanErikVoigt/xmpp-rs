// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use tokio_xmpp::jid::BareJid;
#[cfg(feature = "avatars")]
use tokio_xmpp::jid::Jid;
use tokio_xmpp::parsers::roster::Item as RosterItem;

use crate::{delay::StanzaTimeInfo, Error, MessageId, RoomNick};

#[derive(Debug)]
pub enum Event {
    Online,
    Disconnected(Error),
    ContactAdded(RosterItem),
    ContactRemoved(RosterItem),
    ContactChanged(RosterItem),
    #[cfg(feature = "avatars")]
    AvatarRetrieved(Jid, String),
    /// A chat message was received. It may have been delayed on the network.
    /// - The [`MessageId`] is a unique identifier for this message.
    /// - The [`BareJid`] is the sender's JID.
    /// - The [`String`] is the message body.
    /// - The [`StanzaTimeInfo`] about when message was received, and when the message was claimed sent.
    ChatMessage(Option<MessageId>, BareJid, String, StanzaTimeInfo),
    /// A message in a one-to-one chat was corrected/edited.
    /// - The [`MessageId`] is the ID of the message that was corrected.
    /// - The [`BareJid`] is the JID of the other participant in the chat.
    /// - The [`String`] is the new body of the message, to replace the old one.
    /// - The [`StanzaTimeInfo`] is the time the message correction was sent/received
    ChatMessageCorrection(MessageId, BareJid, String, StanzaTimeInfo),
    RoomJoined(BareJid),
    RoomLeft(BareJid),
    RoomMessage(Option<MessageId>, BareJid, RoomNick, String, StanzaTimeInfo),
    /// A message in a MUC was corrected/edited.
    /// - The [`MessageId`] is the ID of the message that was corrected.
    /// - The [`BareJid`] is the JID of the room where the message was sent.
    /// - The [`RoomNick`] is the nickname of the sender of the message.
    /// - The [`String`] is the new body of the message, to replace the old one.
    /// - The [`StanzaTimeInfo`] is the time the message correction was sent/received
    RoomMessageCorrection(MessageId, BareJid, RoomNick, String, StanzaTimeInfo),
    /// The subject of a room was received.
    /// - The BareJid is the room's address.
    /// - The RoomNick is the nickname of the room member who set the subject.
    /// - The String is the new subject.
    RoomSubject(BareJid, Option<RoomNick>, String, StanzaTimeInfo),
    /// A private message received from a room, containing the message ID, the room's BareJid,
    /// the sender's nickname, and the message body.
    RoomPrivateMessage(Option<MessageId>, BareJid, RoomNick, String, StanzaTimeInfo),
    /// A private message in a MUC was corrected/edited.
    /// - The [`MessageId`] is the ID of the message that was corrected.
    /// - The [`BareJid`] is the JID of the room where the message was sent.
    /// - The [`RoomNick`] is the nickname of the sender of the message.
    /// - The [`String`] is the new body of the message, to replace the old one.
    /// - The [`StanzaTimeInfo`] is the time the message correction was sent/received
    RoomPrivateMessageCorrection(MessageId, BareJid, RoomNick, String, StanzaTimeInfo),
    ServiceMessage(Option<MessageId>, BareJid, String, StanzaTimeInfo),
    HttpUploadedFile(String),
}
