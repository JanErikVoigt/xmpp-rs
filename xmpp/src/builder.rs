// Copyright (c) 2023 xmpp-rs contributors.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[cfg(feature = "starttls")]
use crate::tokio_xmpp::connect::{DnsConfig, StartTlsServerConnector};

use crate::{
    Agent, ClientFeature, ClientType, Config, RoomNick,
    jid::{BareJid, Jid, ResourceRef},
    parsers::{
        disco::{DiscoInfoResult, Feature, Identity},
        ns,
    },
    tokio_xmpp::{Client as TokioXmppClient, connect::ServerConnector, xmlstream::Timeouts},
};

pub struct ClientBuilder<'a, C: ServerConnector> {
    jid: BareJid,
    password: &'a str,
    server_connector: C,
    config: Config,
    features: Vec<ClientFeature>,
    resource: Option<String>,
    timeouts: Timeouts,
}

#[cfg(feature = "starttls")]
impl ClientBuilder<'_, StartTlsServerConnector> {
    pub fn new<'a>(jid: BareJid, password: &'a str) -> ClientBuilder<'a, StartTlsServerConnector> {
        Self::new_with_connector(
            jid.clone(),
            password,
            StartTlsServerConnector(DnsConfig::srv_default_client(jid.domain())),
        )
    }
}

impl<C: ServerConnector> ClientBuilder<'_, C> {
    pub fn new_with_connector<'a>(
        jid: BareJid,
        password: &'a str,
        server_connector: C,
    ) -> ClientBuilder<'a, C> {
        ClientBuilder {
            jid,
            password,
            server_connector,
            config: Config::default(),
            features: vec![],
            resource: None,
            timeouts: Timeouts::default(),
        }
    }

    /// Optionally set a resource associated to this device on the client
    pub fn set_resource(mut self, resource: &str) -> Self {
        self.resource = Some(resource.to_string());
        self
    }

    pub fn set_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn set_client(mut self, type_: ClientType, name: &str) -> Self {
        self.config.disco = (type_, String::from(name));
        self
    }

    pub fn set_website(mut self, url: &str) -> Self {
        self.config.website = String::from(url);
        self
    }

    pub fn set_default_nick(mut self, nick: impl AsRef<ResourceRef>) -> Self {
        self.config.default_nick = RoomNick::from_resource_ref(nick.as_ref());
        self
    }

    pub fn set_lang(mut self, lang: Vec<String>) -> Self {
        self.config.lang = lang;
        self
    }

    /// Configure the timeouts used.
    ///
    /// See [`Timeouts`] for more information on the semantics and the
    /// defaults (which are used unless you call this method).
    pub fn set_timeouts(mut self, timeouts: Timeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    pub fn enable_feature(mut self, feature: ClientFeature) -> Self {
        self.features.push(feature);
        self
    }

    fn make_disco(&self) -> DiscoInfoResult {
        let identities = vec![Identity::new(
            "client",
            self.config.disco.0.to_string(),
            "en",
            self.config.disco.1.to_string(),
        )];
        let mut features = vec![Feature::new(ns::DISCO_INFO)];
        #[cfg(feature = "avatars")]
        {
            if self.features.contains(&ClientFeature::Avatars) {
                features.push(Feature::new(format!("{}+notify", ns::AVATAR_METADATA)));
            }
        }
        if self.features.contains(&ClientFeature::JoinRooms) {
            features.push(Feature::new(format!("{}+notify", ns::BOOKMARKS2)));
        }
        DiscoInfoResult {
            node: None,
            identities,
            features,
            extensions: vec![],
        }
    }

    pub fn build(self) -> Agent {
        let jid: Jid = if let Some(resource) = &self.resource {
            self.jid.with_resource_str(resource).unwrap().into()
        } else {
            self.jid.clone().into()
        };

        let client = TokioXmppClient::new_with_connector(
            jid,
            self.password,
            self.server_connector.clone(),
            self.timeouts,
        );
        self.build_impl(client)
    }

    // This function is meant to be used for testing build
    pub(crate) fn build_impl(self, client: TokioXmppClient) -> Agent {
        let disco = self.make_disco();

        Agent::new(client, self.config, disco)
    }
}
