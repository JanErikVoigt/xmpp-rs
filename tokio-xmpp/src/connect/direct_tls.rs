// Copyright (c) 2025 Saarko <saarko@tutanota.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! `direct_tls::ServerConfig` provides a `ServerConnector` for direct TLS connections

use alloc::borrow::Cow;
use core::{error::Error as StdError, fmt};
#[cfg(feature = "native-tls")]
use native_tls::Error as TlsError;
#[cfg(feature = "rustls-any-backend")]
use tokio_rustls::rustls::pki_types::InvalidDnsNameError;
// Note: feature = "rustls-any-backend" and feature = "native-tls" are
// mutually exclusive during normal compiles, but we allow it for rustdoc
// builds. Thus, we have to make sure that the compilation still succeeds in
// such a case.
#[cfg(all(feature = "rustls-any-backend", not(feature = "native-tls")))]
use tokio_rustls::rustls::Error as TlsError;

#[cfg(all(feature = "rustls-any-backend", not(feature = "native-tls")))]
use {
    alloc::sync::Arc,
    tokio_rustls::{
        rustls::pki_types::ServerName,
        rustls::{ClientConfig, RootCertStore},
        TlsConnector,
    },
};

#[cfg(all(
    feature = "rustls-any-backend",
    not(feature = "ktls"),
    not(feature = "native-tls")
))]
use tokio_rustls::client::TlsStream;

#[cfg(all(feature = "ktls", not(feature = "native-tls")))]
type TlsStream<S> = ktls::KtlsStream<S>;

#[cfg(feature = "native-tls")]
use {
    native_tls::TlsConnector as NativeTlsConnector,
    tokio_native_tls::{TlsConnector, TlsStream},
};

use sasl::common::ChannelBinding;
use tokio::{io::BufStream, net::TcpStream};
use xmpp_parsers::jid::Jid;

use crate::{
    connect::{DnsConfig, ServerConnector, ServerConnectorError},
    error::Error,
    xmlstream::{initiate_stream, PendingFeaturesRecv, StreamHeader, Timeouts},
};

/// Connect via direct TLS to an XMPP server
#[derive(Debug, Clone)]
pub struct DirectTlsServerConnector(pub DnsConfig);

impl From<DnsConfig> for DirectTlsServerConnector {
    fn from(dns_config: DnsConfig) -> DirectTlsServerConnector {
        Self(dns_config)
    }
}

impl ServerConnector for DirectTlsServerConnector {
    type Stream = BufStream<TlsStream<TcpStream>>;

    async fn connect(
        &self,
        jid: &Jid,
        ns: &'static str,
        timeouts: Timeouts,
    ) -> Result<(PendingFeaturesRecv<Self::Stream>, ChannelBinding), Error> {
        let tcp_stream = self.0.resolve().await?;

        // Immediately establish TLS connection
        let (tls_stream, channel_binding) =
            establish_tls(tcp_stream, jid.domain().as_str()).await?;

        // Establish XMPP stream over TLS
        Ok((
            initiate_stream(
                tokio::io::BufStream::new(tls_stream),
                ns,
                StreamHeader {
                    to: Some(Cow::Borrowed(jid.domain().as_str())),
                    // Setting explicitly `from` here, because server require it
                    // in order to advertise i.e. SASL2 (XEP-0388).
                    from: Some(Cow::Borrowed(jid.to_bare().as_str())),
                    id: None,
                },
                timeouts,
            )
            .await?,
            channel_binding,
        ))
    }
}

#[cfg(feature = "native-tls")]
async fn establish_tls(
    tcp_stream: TcpStream,
    domain: &str,
) -> Result<(TlsStream<TcpStream>, ChannelBinding), Error> {
    let domain = domain.to_owned();
    let tls_stream = TlsConnector::from(NativeTlsConnector::builder().build().unwrap())
        .connect(&domain, tcp_stream)
        .await
        .map_err(|e| DirectTlsError::Tls(e))?;
    log::warn!(
        "tls-native doesn't support channel binding, please use tls-rust if you want this feature!"
    );
    Ok((tls_stream, ChannelBinding::None))
}

#[cfg(all(feature = "rustls-any-backend", not(feature = "native-tls")))]
async fn establish_tls(
    tcp_stream: TcpStream,
    domain: &str,
) -> Result<(TlsStream<TcpStream>, ChannelBinding), Error> {
    let domain = ServerName::try_from(domain.to_owned()).map_err(DirectTlsError::DnsNameError)?;
    let mut root_store = RootCertStore::empty();
    #[cfg(feature = "webpki-roots")]
    {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    #[cfg(feature = "rustls-native-certs")]
    {
        root_store.add_parsable_certificates(rustls_native_certs::load_native_certs()?);
    }
    #[allow(unused_mut, reason = "This config is mutable when using ktls")]
    let mut config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    #[cfg(feature = "ktls")]
    let tcp_stream = {
        config.enable_secret_extraction = true;
        ktls::CorkStream::new(tcp_stream)
    };
    let tls_stream = TlsConnector::from(Arc::new(config))
        .connect(domain, tcp_stream)
        .await
        .map_err(crate::Error::Io)?;

    // Extract the channel-binding information before we hand the stream over to ktls.
    let (_, connection) = tls_stream.get_ref();
    let channel_binding = match connection.protocol_version() {
        // TODO: Add support for TLS 1.2 and earlier.
        Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_3) => {
            let data = vec![0u8; 32];
            let data = connection
                .export_keying_material(data, b"EXPORTER-Channel-Binding", None)
                .map_err(DirectTlsError::Tls)?;
            ChannelBinding::TlsExporter(data)
        }
        _ => ChannelBinding::None,
    };

    #[cfg(feature = "ktls")]
    let tls_stream = ktls::config_ktls_client(tls_stream)
        .await
        .map_err(DirectTlsError::KtlsError)?;
    Ok((tls_stream, channel_binding))
}

/// Direct TLS ServerConnector Error
#[derive(Debug)]
pub enum DirectTlsError {
    /// TLS error
    Tls(TlsError),
    #[cfg(feature = "rustls-any-backend")]
    /// DNS name parsing error
    DnsNameError(InvalidDnsNameError),
    #[cfg(feature = "ktls")]
    /// Error while setting up kernel TLS
    KtlsError(ktls::Error),
}

impl ServerConnectorError for DirectTlsError {}

impl fmt::Display for DirectTlsError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Tls(e) => write!(fmt, "TLS error: {}", e),
            #[cfg(feature = "rustls-any-backend")]
            Self::DnsNameError(e) => write!(fmt, "DNS name error: {}", e),
            #[cfg(feature = "ktls")]
            Self::KtlsError(e) => write!(fmt, "Kernel TLS error: {}", e),
        }
    }
}

impl StdError for DirectTlsError {}

impl From<TlsError> for DirectTlsError {
    fn from(e: TlsError) -> Self {
        Self::Tls(e)
    }
}

#[cfg(feature = "rustls-any-backend")]
impl From<InvalidDnsNameError> for DirectTlsError {
    fn from(e: InvalidDnsNameError) -> Self {
        Self::DnsNameError(e)
    }
}
