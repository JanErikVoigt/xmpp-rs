//! `starttls::ServerConfig` provides a `ServerConnector` for starttls connections

use alloc::borrow::Cow;
use core::{error::Error as StdError, fmt};
#[cfg(feature = "tls-native")]
use native_tls::Error as TlsError;
use std::io;
use std::os::fd::AsRawFd;
#[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
use tokio_rustls::rustls::pki_types::InvalidDnsNameError;
#[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
use tokio_rustls::rustls::Error as TlsError;

use futures::{sink::SinkExt, stream::StreamExt};

#[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
use {
    alloc::sync::Arc,
    tokio_rustls::{
        rustls::pki_types::ServerName,
        rustls::{ClientConfig, RootCertStore},
        TlsConnector,
    },
};

#[cfg(all(
    feature = "tls-rust",
    not(feature = "tls-native"),
    not(feature = "tls-rust-ktls")
))]
use tokio_rustls::client::TlsStream;

#[cfg(all(feature = "tls-rust-ktls", not(feature = "tls-native")))]
type TlsStream<S> = ktls::KtlsStream<S>;

#[cfg(feature = "tls-native")]
use {
    native_tls::TlsConnector as NativeTlsConnector,
    tokio_native_tls::{TlsConnector, TlsStream},
};

use sasl::common::ChannelBinding;
use tokio::{
    io::{AsyncRead, AsyncWrite, BufStream},
    net::TcpStream,
};
use xmpp_parsers::{
    jid::Jid,
    starttls::{self, Request},
};

use crate::{
    connect::{DnsConfig, ServerConnector, ServerConnectorError},
    error::{Error, ProtocolError},
    xmlstream::{
        initiate_stream, PendingFeaturesRecv, ReadError, StreamHeader, Timeouts, XmppStream,
        XmppStreamElement,
    },
    Client,
};

/// Client that connects over StartTls
#[deprecated(since = "5.0.0", note = "use tokio_xmpp::Client instead")]
pub type StartTlsClient = Client;

/// Connect via TCP+StartTLS to an XMPP server
#[derive(Debug, Clone)]
pub struct StartTlsServerConnector(pub DnsConfig);

impl From<DnsConfig> for StartTlsServerConnector {
    fn from(dns_config: DnsConfig) -> StartTlsServerConnector {
        Self(dns_config)
    }
}

impl ServerConnector for StartTlsServerConnector {
    type Stream = BufStream<TlsStream<TcpStream>>;

    async fn connect(
        &self,
        jid: &Jid,
        ns: &'static str,
        timeouts: Timeouts,
    ) -> Result<(PendingFeaturesRecv<Self::Stream>, ChannelBinding), Error> {
        let tcp_stream = tokio::io::BufStream::new(self.0.resolve().await?);

        // Unencryped XmppStream
        let xmpp_stream = initiate_stream(
            tcp_stream,
            ns,
            StreamHeader {
                to: Some(Cow::Borrowed(jid.domain().as_str())),
                from: None,
                id: None,
            },
            timeouts,
        )
        .await?;
        let (features, xmpp_stream) = xmpp_stream.recv_features().await?;

        if features.can_starttls() {
            // TlsStream
            let (tls_stream, channel_binding) =
                starttls(xmpp_stream, jid.domain().as_str()).await?;
            // Encrypted XmppStream
            Ok((
                initiate_stream(
                    tokio::io::BufStream::new(tls_stream),
                    ns,
                    StreamHeader {
                        to: Some(Cow::Borrowed(jid.domain().as_str())),
                        from: None,
                        id: None,
                    },
                    timeouts,
                )
                .await?,
                channel_binding,
            ))
        } else {
            Err(crate::Error::Protocol(ProtocolError::NoTls))
        }
    }
}

#[cfg(feature = "tls-native")]
async fn get_tls_stream<S: AsyncRead + AsyncWrite + Unpin>(
    xmpp_stream: XmppStream<BufStream<S>>,
    domain: &str,
) -> Result<(TlsStream<S>, ChannelBinding), Error> {
    let domain = domain.to_owned();
    let stream = xmpp_stream.into_inner().into_inner();
    let tls_stream = TlsConnector::from(NativeTlsConnector::builder().build().unwrap())
        .connect(&domain, stream)
        .await
        .map_err(|e| StartTlsError::Tls(e))?;
    log::warn!(
        "tls-native doesn’t support channel binding, please use tls-rust if you want this feature!"
    );
    Ok((tls_stream, ChannelBinding::None))
}

#[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
async fn get_tls_stream<S: AsyncRead + AsyncWrite + Unpin + AsRawFd>(
    xmpp_stream: XmppStream<BufStream<S>>,
    domain: &str,
) -> Result<(TlsStream<S>, ChannelBinding), Error> {
    let domain = ServerName::try_from(domain.to_owned()).map_err(StartTlsError::DnsNameError)?;
    let stream = xmpp_stream.into_inner().into_inner();
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
    #[cfg(feature = "tls-rust-ktls")]
    let stream = {
        config.enable_secret_extraction = true;
        ktls::CorkStream::new(stream)
    };
    let tls_stream = TlsConnector::from(Arc::new(config))
        .connect(domain, stream)
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
                .map_err(StartTlsError::Tls)?;
            ChannelBinding::TlsExporter(data)
        }
        _ => ChannelBinding::None,
    };

    #[cfg(feature = "tls-rust-ktls")]
    let tls_stream = ktls::config_ktls_client(tls_stream)
        .await
        .map_err(StartTlsError::KtlsError)?;
    Ok((tls_stream, channel_binding))
}

/// Performs `<starttls/>` on an XmppStream and returns a binary
/// TlsStream.
pub async fn starttls<S: AsyncRead + AsyncWrite + Unpin + AsRawFd>(
    mut stream: XmppStream<BufStream<S>>,
    domain: &str,
) -> Result<(TlsStream<S>, ChannelBinding), Error> {
    stream
        .send(&XmppStreamElement::Starttls(starttls::Nonza::Request(
            Request,
        )))
        .await?;

    loop {
        match stream.next().await {
            Some(Ok(XmppStreamElement::Starttls(starttls::Nonza::Proceed(_)))) => {
                break;
            }
            Some(Ok(_)) => (),
            Some(Err(ReadError::SoftTimeout)) => (),
            Some(Err(ReadError::HardError(e))) => return Err(e.into()),
            Some(Err(ReadError::ParseError(e))) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, e).into())
            }
            None | Some(Err(ReadError::StreamFooterReceived)) => {
                return Err(crate::Error::Disconnected)
            }
        }
    }

    get_tls_stream(stream, domain).await
}

/// StartTLS ServerConnector Error
#[derive(Debug)]
pub enum StartTlsError {
    /// TLS error
    Tls(TlsError),
    #[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
    /// DNS name parsing error
    DnsNameError(InvalidDnsNameError),
    #[cfg(feature = "tls-rust-ktls")]
    /// Error while setting up kernel TLS
    KtlsError(ktls::Error),
}

impl ServerConnectorError for StartTlsError {}

impl fmt::Display for StartTlsError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Tls(e) => write!(fmt, "TLS error: {}", e),
            #[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
            Self::DnsNameError(e) => write!(fmt, "DNS name error: {}", e),
            #[cfg(feature = "tls-rust-ktls")]
            Self::KtlsError(e) => write!(fmt, "Kernel TLS error: {}", e),
        }
    }
}

impl StdError for StartTlsError {}

impl From<TlsError> for StartTlsError {
    fn from(e: TlsError) -> Self {
        Self::Tls(e)
    }
}

#[cfg(all(feature = "tls-rust", not(feature = "tls-native")))]
impl From<InvalidDnsNameError> for StartTlsError {
    fn from(e: InvalidDnsNameError) -> Self {
        Self::DnsNameError(e)
    }
}
