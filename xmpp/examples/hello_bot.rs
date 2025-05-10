// Copyright (c) 2019 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[cfg(feature = "rustls-any-backend")]
use xmpp::tokio_xmpp::rustls;
use xmpp::{
    jid::BareJid,
    muc::room::{JoinRoomSettings, RoomMessageSettings},
    Agent, ClientBuilder, ClientFeature, ClientType, Event, RoomNick,
};

use tokio::signal::ctrl_c;

use std::env::args;
use std::str::FromStr;

#[cfg(all(
    feature = "rustls-any-backend",
    not(any(feature = "aws_lc_rs", feature = "ring"))
))]
compile_error!("using rustls (e.g. via the ktls feature) needs an enabled rustls backend feature (either aws_lc_rs or ring).");

#[tokio::main]
async fn main() -> Result<(), Option<()>> {
    env_logger::init();

    #[cfg(all(feature = "aws_lc_rs", not(feature = "ring")))]
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    #[cfg(all(feature = "ring"))]
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

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
        tokio::select! {
            events = client.wait_for_events() => {
                for event in events {
                    let _ = handle_events(&mut client, event, &rooms);
                }
            },
            _ = ctrl_c() => {
                log::info!("Disconnecting...");
                let _ = client.disconnect().await;
                break;
            },
        }
    }

    Ok(())
}

async fn handle_events(client: &mut Agent, event: Event, rooms: &Vec<BareJid>) {
    match event {
        Event::Online => {
            log::info!("Online.");
            for room in rooms {
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
                body
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
                jid, nick, time_info.received, body
            );
        }
        _ => {
            log::debug!("Unimplemented event:\n{:#?}", event);
        }
    }
}
