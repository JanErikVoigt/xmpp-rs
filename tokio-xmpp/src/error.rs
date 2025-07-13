use core::{error::Error as StdError, fmt, net::AddrParseError, str::Utf8Error};
#[cfg(feature = "dns")]
use hickory_resolver::{
    error::ResolveError as DnsResolveError, proto::error::ProtoError as DnsProtoError,
};
use sasl::client::MechanismError as SaslMechanismError;
use std::io;

use xmpp_parsers::stream_error::ReceivedStreamError;

use crate::{
    connect::ServerConnectorError, jid, minidom,
    parsers::sasl::DefinedCondition as SaslDefinedCondition, xmlstream::RecvFeaturesError,
};

/// Top-level error type
#[derive(Debug)]
pub enum Error {
    /// I/O error
    Io(io::Error),
    /// Error parsing Jabber-Id
    JidParse(jid::Error),
    /// Protocol-level error
    Protocol(ProtocolError),
    /// Authentication error
    Auth(AuthError),
    /// Connection closed
    Disconnected,
    /// Should never happen
    InvalidState,
    /// Fmt error
    Fmt(fmt::Error),
    /// Utf8 error
    Utf8(Utf8Error),
    /// Error specific to ServerConnector impl
    Connection(Box<dyn ServerConnectorError>),
    /// DNS protocol error
    #[cfg(feature = "dns")]
    Dns(DnsProtoError),
    /// DNS resolution error
    #[cfg(feature = "dns")]
    Resolve(DnsResolveError),
    /// DNS label conversion error, no details available from module
    /// `idna`
    #[cfg(feature = "dns")]
    Idna,
    /// Invalid IP/Port address
    Addr(AddrParseError),
    /// Received a stream error
    StreamError(ReceivedStreamError),
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(fmt, "IO error: {}", e),
            Error::Connection(e) => write!(fmt, "connection error: {}", e),
            Error::JidParse(e) => write!(fmt, "jid parse error: {}", e),
            Error::Protocol(e) => write!(fmt, "protocol error: {}", e),
            Error::Auth(e) => write!(fmt, "authentication error: {}", e),
            Error::Disconnected => write!(fmt, "disconnected"),
            Error::InvalidState => write!(fmt, "invalid state"),
            Error::Fmt(e) => write!(fmt, "Fmt error: {}", e),
            Error::Utf8(e) => write!(fmt, "Utf8 error: {}", e),
            #[cfg(feature = "dns")]
            Error::Dns(e) => write!(fmt, "{:?}", e),
            #[cfg(feature = "dns")]
            Error::Resolve(e) => write!(fmt, "{:?}", e),
            #[cfg(feature = "dns")]
            Error::Idna => write!(fmt, "IDNA error"),
            Error::Addr(e) => write!(fmt, "Wrong network address: {e}"),
            Error::StreamError(e) => write!(fmt, "{e}"),
        }
    }
}

impl StdError for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl<T: ServerConnectorError + 'static> From<T> for Error {
    fn from(e: T) -> Self {
        Error::Connection(Box::new(e))
    }
}

impl From<jid::Error> for Error {
    fn from(e: jid::Error) -> Self {
        Error::JidParse(e)
    }
}

impl From<ProtocolError> for Error {
    fn from(e: ProtocolError) -> Self {
        Error::Protocol(e)
    }
}

impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Error::Auth(e)
    }
}

impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Error::Fmt(e)
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Self {
        Error::Utf8(e)
    }
}

#[cfg(feature = "dns")]
impl From<idna::Errors> for Error {
    fn from(_e: idna::Errors) -> Self {
        Error::Idna
    }
}

#[cfg(feature = "dns")]
impl From<DnsResolveError> for Error {
    fn from(e: DnsResolveError) -> Error {
        Error::Resolve(e)
    }
}

#[cfg(feature = "dns")]
impl From<DnsProtoError> for Error {
    fn from(e: DnsProtoError) -> Error {
        Error::Dns(e)
    }
}

impl From<AddrParseError> for Error {
    fn from(e: AddrParseError) -> Error {
        Error::Addr(e)
    }
}

impl From<RecvFeaturesError> for Error {
    fn from(e: RecvFeaturesError) -> Self {
        match e {
            RecvFeaturesError::Io(e) => e.into(),
            RecvFeaturesError::StreamError(e) => Self::StreamError(e),
        }
    }
}

/// XMPP protocol-level error
#[derive(Debug)]
pub enum ProtocolError {
    /// XML parser error
    Parser(minidom::Error),
    /// Error with expected stanza schema
    Parsers(xso::error::Error),
    /// No TLS available
    NoTls,
    /// Invalid response to resource binding
    InvalidBindResponse,
    /// No xmlns attribute in <stream:stream>
    NoStreamNamespace,
    /// No id attribute in <stream:stream>
    NoStreamId,
    /// Encountered an unexpected XML token
    InvalidToken,
    /// Unexpected <stream:stream> (shouldn't occur)
    InvalidStreamStart,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProtocolError::Parser(e) => write!(fmt, "XML parser error: {}", e),
            ProtocolError::Parsers(e) => write!(fmt, "error with expected stanza schema: {}", e),
            ProtocolError::NoTls => write!(fmt, "no TLS available"),
            ProtocolError::InvalidBindResponse => {
                write!(fmt, "invalid response to resource binding")
            }
            ProtocolError::NoStreamNamespace => {
                write!(fmt, "no xmlns attribute in <stream:stream>")
            }
            ProtocolError::NoStreamId => write!(fmt, "no id attribute in <stream:stream>"),
            ProtocolError::InvalidToken => write!(fmt, "encountered an unexpected XML token"),
            ProtocolError::InvalidStreamStart => write!(fmt, "unexpected <stream:stream>"),
        }
    }
}

impl StdError for ProtocolError {}

impl From<minidom::Error> for ProtocolError {
    fn from(e: minidom::Error) -> Self {
        ProtocolError::Parser(e)
    }
}

impl From<minidom::Error> for Error {
    fn from(e: minidom::Error) -> Self {
        ProtocolError::Parser(e).into()
    }
}

impl From<xso::error::Error> for ProtocolError {
    fn from(e: xso::error::Error) -> Self {
        ProtocolError::Parsers(e)
    }
}

/// Authentication error
#[derive(Debug)]
pub enum AuthError {
    /// No matching SASL mechanism available
    NoMechanism,
    /// Local SASL implementation error
    Sasl(SaslMechanismError),
    /// Failure from server
    Fail(SaslDefinedCondition),
    /// Component authentication failure
    ComponentFail,
}

impl StdError for AuthError {}

impl fmt::Display for AuthError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthError::NoMechanism => write!(fmt, "no matching SASL mechanism available"),
            AuthError::Sasl(s) => write!(fmt, "local SASL implementation error: {}", s),
            AuthError::Fail(c) => write!(fmt, "failure from the server: {:?}", c),
            AuthError::ComponentFail => write!(fmt, "component authentication failure"),
        }
    }
}
