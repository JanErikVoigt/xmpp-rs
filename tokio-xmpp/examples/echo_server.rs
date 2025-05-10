use futures::{SinkExt, StreamExt};
use tokio::{self, io, net::TcpSocket};

#[cfg(feature = "rustls-any-backend")]
use tokio_xmpp::rustls;
use tokio_xmpp::{
    minidom::Element,
    parsers::stream_features::StreamFeatures,
    xmlstream::{accept_stream, StreamHeader, Timeouts},
};

#[cfg(all(
    feature = "rustls-any-backend",
    not(any(feature = "aws_lc_rs", feature = "ring"))
))]
compile_error!("using rustls (e.g. via the ktls feature) needs an enabled rustls backend feature (either aws_lc_rs or ring).");

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    #[cfg(all(feature = "aws_lc_rs", not(feature = "ring")))]
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    #[cfg(all(feature = "ring"))]
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    // TCP socket
    let address = "127.0.0.1:5222".parse().unwrap();
    let socket = TcpSocket::new_v4()?;
    socket.bind(address)?;

    let listener = socket.listen(1024)?;

    // Main loop, accepts incoming connections
    loop {
        let (stream, _addr) = listener.accept().await?;
        let stream = accept_stream(
            tokio::io::BufStream::new(stream),
            tokio_xmpp::parsers::ns::DEFAULT_NS,
            Timeouts::default(),
        )
        .await?;
        let stream = stream.send_header(StreamHeader::default()).await?;
        let mut stream = stream
            .send_features::<Element>(&StreamFeatures::default())
            .await?;

        tokio::spawn(async move {
            while let Some(packet) = stream.next().await {
                match packet {
                    Ok(packet) => {
                        println!("Received packet: {:?}", packet);
                        stream.send(&packet).await.unwrap();
                    }
                    Err(e) => {
                        eprintln!("Error: {:?}", e);
                    }
                }
            }
        });
    }
}
