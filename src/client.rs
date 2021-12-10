use anyhow::Result;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use rust_kvstore::{
    noise_codec::{NoiseCodec, NOISE_PARAMS},
    pb::{Request, Response},
};
use tokio::net::TcpStream;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let addr = "localhost:8888";
    let stream = TcpStream::connect(addr).await?;
    let mut stream = NoiseCodec::builder(NOISE_PARAMS, true).new_framed(stream)?;

    // -> e
    stream.send(Bytes::from_static(&[])).await?;
    info!("-> e");

    // <- e, ee, s, es
    let data = stream.next().await.unwrap()?;
    info!("<- e, ee, s, es");

    // -> s, se
    stream.send(data.freeze()).await?;
    info!("-> s, se");

    stream.codec_mut().into_transport_mode()?;

    let msg = Request::new_put("hello", b"world");
    stream.send(msg.into()).await?;

    let msg = Request::new_get("hello");
    stream.send(msg.into()).await?;

    while let Some(Ok(buf)) = stream.next().await {
        let msg = Response::try_from(buf)?;
        println!("Got msg: {:?}", msg);
    }

    Ok(())
}
