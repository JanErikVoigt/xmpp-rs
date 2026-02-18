use core::{fmt, net::SocketAddr};
use hickory_resolver::config::LookupIpStrategy;
use hickory_resolver::proto::rr::RData;

#[cfg(feature = "dns")]
use futures::{future::select_ok, FutureExt};
#[cfg(feature = "dns")]
#[cfg(feature = "dns")]
use log::debug;

use crate::Error;
use hickory_resolver::proto::rr::domain::IntoName;
use tokio::net::TcpStream;

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
        use hickory_resolver::TokioResolver;

        let ascii_domain = idna::domain_to_ascii(host)?;

        if let Ok(ip) = ascii_domain.parse() {
            debug!("Attempting connection to {ip}:{fallback_port}");
            return Ok(TcpStream::connect(&SocketAddr::new(ip, fallback_port)).await?);
        }

        let resolver: Resolver<_> = TokioResolver::builder_tokio()?.build()?;

        let srv_domain = format!("{}.{}", srv, ascii_domain).into_name()?;
        let srv_records = resolver.srv_lookup(srv_domain.clone()).await.ok();

        match srv_records {
            Some(lookup) => {
                // TODO: sort lookup records by priority/weight
                for record in lookup.answers() {
                    debug!("Attempting connection to {srv_domain} {record:?}");
                    println!("Attempting connection to {srv_domain} {record:?}\n {record}");

                    if let RData::SRV(srv) = record.data() {
                        let port = srv.port();
                        let target = srv.target().to_utf8(); // or to_ascii()

                        println!("try to connect to: Target={target}, Port={port}");

                        if let Ok(stream) = Self::resolve_no_srv(&target, port).await {
                            return Ok(stream);
                        }
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

        let mut builder = TokioResolver::builder_tokio()?;
        builder.options_mut().ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
        let resolver = builder.build()?;

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

#[cfg(test)]
mod dns_tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    const TIMEOUT: Duration = Duration::from_secs(4);

    //#[test]
    #[tokio::test]
    async fn resolve_not_using_srv() {
        let config = DnsConfig::NoSrv {
            host: "google.com".to_ascii_lowercase(),
            port: 8080,
        };
        let res = timeout(TIMEOUT, config.resolve()).await;
        println!("{:?}", res);
        assert!(res.is_ok());
        assert!(res.unwrap().is_ok());
    }
}
