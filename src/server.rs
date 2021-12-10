use anyhow::Result;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use rust_kvstore::noise_codec::{NoiseCodec, NOISE_PARAMS};
use rust_kvstore::pb::request::Command;
use rust_kvstore::pb::{Request, RequestGet, RequestPut, Response};
use std::convert::TryInto;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Debug)]
struct ServerState {
    store: DashMap<String, Vec<u8>>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
        }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state = Arc::new(ServerState::new());
    let addr = "0.0.0.0:8888";
    let listener = TcpListener::bind(addr).await?;
    info!("Listen to {:?}", addr);

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("New client: {:?} accepted", addr);
        let shared = state.clone();

        tokio::spawn(async move {
            let mut stream = NoiseCodec::builder(NOISE_PARAMS, false).new_framed(stream)?;

            // <- e
            let data = stream.next().await.unwrap()?;
            info!("<- e");

            // -> e, ee, s, es
            stream.send(data.freeze()).await?;
            info!("-> e, ee, s, es");

            //<- s, se
            let _data = stream.next().await.unwrap()?;
            info!("<- s, se");

            stream.codec_mut().into_transport_mode()?;

            while let Some(Ok(buf)) = stream.next().await {
                let msg: Request = buf.try_into()?;
                info!("Got a command: {:?}", msg);
                let response = match msg.command {
                    Some(Command::Get(RequestGet { key })) => match shared.store.get(&key) {
                        Some(v) => Response::new(key, v.value().to_vec()),
                        None => Response::not_found(key),
                    },
                    Some(Command::Put(RequestPut { key, value })) => {
                        shared.store.insert(key.clone(), value.clone());
                        Response::new(key, value)
                    }
                    None => unimplemented!(),
                };

                stream.send(response.into()).await?;
            }

            Ok::<(), anyhow::Error>(())
        });
    }
}
