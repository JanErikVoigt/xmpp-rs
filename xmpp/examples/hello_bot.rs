// Copyright (c) 2019 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use xmpp::{
    jid::BareJid,
    muc::room::{JoinRoomSettings, RoomMessageSettings},
    ClientBuilder, ClientFeature, ClientType, Event, RoomNick,
};

use std::env::args;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Option<()>> {
    env_logger::init();

    let args: Vec<String> = args().collect();
    if args.len() < 3 {
        println!("Usage: {} <jid> <password> [ROOM...]", args[0]);
        return Err(None);
    }

    let jid = BareJid::from_str(&args[1]).expect(&format!("Invalid JID: {}", &args[1]));
    let password = &args[2];

    // Figure out which rooms to join to say hello
    let mut rooms: Vec<BareJid> = Vec::new();
    let mut counter = 3;
    if args.len() > 3 {
        while counter < args.len() {
            match BareJid::from_str(&args[counter]) {
                Ok(jid) => rooms.push(jid),
                Err(e) => {
                    log::error!("Requested room {} is not a valid JID: {e}", args[counter]);
                    std::process::exit(1);
                }
            }
            counter += 1;
        }
    }

    let nick = RoomNick::from_str("bot").unwrap();

    // Client instance
    let mut client = ClientBuilder::new(jid, password)
        .set_client(ClientType::Bot, "xmpp-rs")
        .set_website("https://xmpp.rs")
        .set_default_nick(nick)
        .enable_feature(ClientFeature::ContactList)
        .enable_feature(ClientFeature::JoinRooms)
        .build();

    log::info!("Connecting...");

    loop {
        for event in client.wait_for_events().await {
            match event {
                Event::Online => {
                    log::info!("Online.");
                    for room in &rooms {
                        log::info!("Joining room {} from CLI argument…", room);
                        client
                            .join_room(JoinRoomSettings {
                                room: room.clone(),
                                nick: None,
                                password: None,
                                status: Some(("en", "Yet another bot!")),
                            })
                            .await;
                    }
                }
                Event::Disconnected(e) => {
                    log::info!("Disconnected: {}.", e);
                }
                Event::ChatMessage(_id, jid, body, time_info) => {
                    log::info!(
                        "{} {}: {}",
                        time_info.received.time().format("%H:%M"),
                        jid,
                        body.0
                    );
                }
                Event::RoomJoined(jid) => {
                    log::info!("Joined room {}.", jid);
                    client
                        .send_room_message(RoomMessageSettings::new(jid, "Hello world!"))
                        .await;
                }
                Event::RoomMessage(_id, jid, nick, body, time_info) => {
                    println!(
                        "Message in room {} from {} at {}: {}",
                        jid, nick, time_info.received, body.0
                    );
                }
                _ => {
                    log::debug!("Unimplemented event:\n{:#?}", event);
                }
            }
        }
    }
}
