use core::{fmt, net::SocketAddr};
use hickory_resolver::config::LookupIpStrategy;

#[cfg(feature = "dns")]
use futures::{future::select_ok, FutureExt};
#[cfg(feature = "dns")]
#[cfg(feature = "dns")]
use log::debug;
use tokio::net::TcpStream;

use crate::Error;
use hickory_resolver::proto::rr::domain::IntoName;

/// XMPP server connection configuration
#[derive(Clone, Debug)]
pub enum DnsConfig {
    /// Use SRV record to find server host
    #[cfg(feature = "dns")]
    UseSrv {
        /// Hostname to resolve
        host: String,
        /// TXT field eg. _xmpp-client._tcp
        srv: String,
        /// When SRV resolution fails what port to use
        fallback_port: u16,
    },

    /// Manually define server host and port
    #[allow(unused)]
    #[cfg(feature = "dns")]
    NoSrv {
        /// Server host name
        host: String,
        /// Server port
        port: u16,
    },

    /// Manually define IP: port (TODO: socket)
    #[allow(unused)]
    Addr {
        /// IP:port
        addr: String,
    },
}

impl fmt::Display for DnsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "dns")]
            Self::UseSrv { host, .. } => write!(f, "{}", host),
            #[cfg(feature = "dns")]
            Self::NoSrv { host, port } => write!(f, "{}:{}", host, port),
            Self::Addr { addr } => write!(f, "{}", addr),
        }
    }
}

impl DnsConfig {
    /// Constructor for DnsConfig::UseSrv variant
    #[cfg(feature = "dns")]
    pub fn srv(host: &str, srv: &str, fallback_port: u16) -> Self {
        Self::UseSrv {
            host: host.to_string(),
            srv: srv.to_string(),
            fallback_port,
        }
    }

    /// Constructor for the default SRV resolution strategy for clients (StartTLS)
    #[cfg(feature = "dns")]
    pub fn srv_default_client(host: &str) -> Self {
        Self::UseSrv {
            host: host.to_string(),
            srv: "_xmpp-client._tcp".to_string(),
            fallback_port: 5222,
        }
    }

    /// Constructor for direct TLS connections using RFC 7590 _xmpps-client._tcp
    #[cfg(feature = "dns")]
    pub fn srv_xmpps(host: &str) -> Self {
        Self::UseSrv {
            host: host.to_string(),
            srv: "_xmpps-client._tcp".to_string(),
            fallback_port: 5223,
        }
    }

    /// Constructor for DnsConfig::NoSrv variant
    #[cfg(feature = "dns")]
    pub fn no_srv(host: &str, port: u16) -> Self {
        Self::NoSrv {
            host: host.to_string(),
            port,
        }
    }

    /// Constructor for DnsConfig::Addr variant
    pub fn addr(addr: &str) -> Self {
        Self::Addr {
            addr: addr.to_string(),
        }
    }

    /// Try resolve the DnsConfig to a TcpStream
    pub async fn resolve(&self) -> Result<TcpStream, Error> {
        match self {
            #[cfg(feature = "dns")]
            Self::UseSrv {
                host,
                srv,
                fallback_port,
            } => Self::resolve_srv(host, srv, *fallback_port).await,
            #[cfg(feature = "dns")]
            Self::NoSrv { host, port } => Self::resolve_no_srv(host, *port).await,
            Self::Addr { addr } => {
                // TODO: Unix domain socket
                let addr: SocketAddr = addr.parse()?;
                return Ok(TcpStream::connect(&SocketAddr::new(addr.ip(), addr.port())).await?);
            }
        }
    }

    #[cfg(feature = "dns")]
    async fn resolve_srv(host: &str, srv: &str, fallback_port: u16) -> Result<TcpStream, Error> {
        use hickory_resolver::{Resolver, TokioResolver};

        let ascii_domain = idna::domain_to_ascii(host)?;

        if let Ok(ip) = ascii_domain.parse() {
            debug!("Attempting connection to {ip}:{fallback_port}");
            return Ok(TcpStream::connect(&SocketAddr::new(ip, fallback_port)).await?);
        }

        let (_config, options) = hickory_resolver::system_conf::read_system_conf()?;
        let resolver: Resolver<_> = TokioResolver::builder_tokio()?
            .with_options(options)
            .build()?;

        let srv_domain = format!("{}.{}.", srv, ascii_domain).into_name()?;
        let srv_records = resolver.srv_lookup(srv_domain.clone()).await.ok();

        match srv_records {
            Some(lookup) => {
                // TODO: sort lookup records by priority/weight
                for srv in lookup.answers() {
                    debug!("Attempting connection to {srv_domain} {srv}");
                    if let Ok(stream) =
                        Self::resolve_no_srv(&srv.target().to_ascii(), srv.port()).await
                    {
                        return Ok(stream);
                    }
                }
                Err(Error::Disconnected)
            }
            None => {
                // SRV lookup error, retry with hostname
                debug!("Attempting connection to {host}:{fallback_port}");
                Self::resolve_no_srv(host, fallback_port).await
            }
        }
    }

    #[cfg(feature = "dns")]
    async fn resolve_no_srv(host: &str, port: u16) -> Result<TcpStream, Error> {
        use hickory_resolver::TokioResolver;

        let ascii_domain = idna::domain_to_ascii(host)?;

        if let Ok(ip) = ascii_domain.parse() {
            return Ok(TcpStream::connect(&SocketAddr::new(ip, port)).await?);
        }

        let (_config, mut options) = hickory_resolver::system_conf::read_system_conf()?;
        options.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
        let resolver = TokioResolver::builder_tokio()?
            .with_options(options)
            .build()?;

        let ips = resolver.lookup_ip(ascii_domain).await?;

        // Happy Eyeballs: connect to all records in parallel, return the
        // first to succeed
        select_ok(
            ips.iter()
                .map(|ip| TcpStream::connect(SocketAddr::new(ip, port)).boxed()),
        )
        .await
        .map(|(result, _)| result)
        .map_err(|_| Error::Disconnected)
    }
}
