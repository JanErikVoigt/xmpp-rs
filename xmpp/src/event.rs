// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use tokio_xmpp::jid::BareJid;
#[cfg(feature = "avatars")]
use tokio_xmpp::jid::Jid;
use tokio_xmpp::parsers::roster::Item as RosterItem;

use crate::parsers::confirm::Confirm;
use crate::{delay::StanzaTimeInfo, Error, MessageId, RoomNick};

/// An Event notifying the client something has happened that may require attention.
///
/// This can be an XMPP event received from the server, or existing state communicated by the server to the client, like when receiving the contact list.
#[derive(Debug)]
pub enum Event {
    /// Client connected.
    Online,
    /// Client disconnected; if reconnect is disabled, no more event will be received.
    Disconnected(Error),
    /// Contact received from contact list (roster).
    ///
    /// This happens when:
    ///
    /// - it was added recently to the contact list
    /// - or when the client just came online and is receiving the existing contact list
    ContactAdded(RosterItem),
    /// Contact removed from contact list (roster).
    ContactRemoved(RosterItem),
    /// Contact changed in contact list (roster).
    ///
    /// This happens when (non-exhaustive):
    ///
    /// - the contact's nickname changed
    /// - the contact's subscription status changed (eg. they accepted a friend request)
    /// - the contact has been added to or removed from a contact group
    ContactChanged(RosterItem),
    #[cfg(feature = "avatars")]
    /// Avatar received for a certain JID, with sender JID / avatar path
    ///
    /// The avatar path is relative file path to the avatar data.
    ///
    /// NOTE: For now, it's not possible to configure where the avatars are stored, see [issue 112](https://gitlab.com/xmpp-rs/xmpp-rs/-/issues/112) for more information.
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
    /// A XEP-0070 authentication request or confirmation was received.
    /// - The [`BareJid`] is the sender's JID.
    /// - The [`Confirm`] is the info about the authentication request.
    /// - The [`StanzaTimeInfo`] about when message was received, and when the message was claimed sent.
    AuthConfirm(BareJid, Confirm, StanzaTimeInfo),
    /// A XEP-0070 authentication rejection was received.
    /// - The [`BareJid`] is the sender's JID.
    /// - The [`Confirm`] is the info about the authentication request that was rejected.
    /// - The [`StanzaTimeInfo`] about when message was received, and when the message was claimed sent.
    AuthReject(BareJid, Confirm, StanzaTimeInfo),
    /// Room joined; client may receive and send messages from/to this BareJid.
    RoomJoined(BareJid),
    /// Room left; client may not receive and send messages from/to this BareJid
    RoomLeft(BareJid),
    /// Room message received with:
    /// - An optional [`MessageId`] for the message.
    /// - The [`BareJid`] of the room the message was sent from.
    /// - The [`RoomNick`] of the sender.
    /// - A [`String`] containing the actual message.
    /// - The [`StanzaTimeInfo`] containing time related information for the message.
    RoomMessage(Option<MessageId>, BareJid, RoomNick, String, StanzaTimeInfo),
    /// A message in a MUC was corrected/edited.
    /// - The [`MessageId`] is the ID of the message that was corrected.
    /// - The [`BareJid`] is the JID of the room where the message was sent.
    /// - The [`RoomNick`] is the nickname of the sender of the message.
    /// - The [`String`] is the new body of the message, to replace the old one.
    /// - The [`StanzaTimeInfo`] is the time the message correction was sent/received
    RoomMessageCorrection(MessageId, BareJid, RoomNick, String, StanzaTimeInfo),
    /// The subject of a room was received.
    /// - The [`BareJid`] is the room's address.
    /// - The [`RoomNick`] is the nickname of the room member who set the subject.
    /// - The [`String`] is the new subject.
    RoomSubject(BareJid, Option<RoomNick>, String, StanzaTimeInfo),
    /// A private message received from a room, containing:
    /// - An optional [`MessageId`] for the message.
    /// - The room's [`BareJid`].
    /// - The sender's [`RoomNick`].
    /// - A [`String`] containing the actual message.
    /// - The [`StanzaTimeInfo`] containing time related information for the message.
    RoomPrivateMessage(Option<MessageId>, BareJid, RoomNick, String, StanzaTimeInfo),
    /// A private message in a MUC was corrected/edited.
    /// - The [`MessageId`] is the ID of the message that was corrected.
    /// - The [`BareJid`] is the JID of the room where the message was sent.
    /// - The [`RoomNick`] is the nickname of the sender of the message.
    /// - The [`String`] is the new body of the message, to replace the old one.
    /// - The [`StanzaTimeInfo`] is the time the message correction was sent/received
    RoomPrivateMessageCorrection(MessageId, BareJid, RoomNick, String, StanzaTimeInfo),
    /// Service message (eg. server notification) received, with:
    /// - An optional [`MessageId`] for the message.
    /// - The [`BareJid`] of the entity that sent it.
    /// - A [`String`] containing the actual message.
    /// - The [`StanzaTimeInfo`] containing time related information for the message.
    ServiceMessage(Option<MessageId>, BareJid, String, StanzaTimeInfo),
    /// A file has been uploaded over HTTP; contains the URL of the file.
    HttpUploadedFile(String),
    #[cfg(feature = "escape-hatch")]
    TokioXmppEvent(TokioXmppEvent),
}
