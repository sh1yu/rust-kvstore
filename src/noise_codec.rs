use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use snow::{HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Decoder, Encoder, Framed};

pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const HEADER_LEN: usize = 2;
const MAX_FRAME_SIZE: usize = 65535;

enum NoiseState {
    HandShake(HandshakeState),
    Transport(TransportState),
    None,
}

impl NoiseState {
    fn write_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, snow::Error> {
        match self {
            NoiseState::HandShake(s) => s.write_message(payload, message),
            NoiseState::Transport(s) => s.write_message(payload, message),
            NoiseState::None => unimplemented!(),
        }
    }

    fn read_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, snow::Error> {
        match self {
            NoiseState::HandShake(s) => s.read_message(payload, message),
            NoiseState::Transport(s) => s.read_message(payload, message),
            NoiseState::None => unimplemented!(),
        }
    }
}

#[allow(dead_code)]
pub struct NoiseCodec {
    builder: Builder,
    state: NoiseState,
}

impl NoiseCodec {
    pub fn builder(params: &'static str, initiator: bool) -> Builder {
        Builder::new(params, initiator)
    }

    pub fn into_transport_mode(&mut self) -> Result<(), snow::Error> {
        self.state = match std::mem::replace(&mut self.state, NoiseState::None) {
            NoiseState::HandShake(s) => NoiseState::Transport(s.into_transport_mode()?),
            v => v,
        };

        Ok(())
    }
}

impl Encoder<Bytes> for NoiseCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: Bytes, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        let mut buf = [0u8; MAX_FRAME_SIZE];
        let n = item.len();

        if n > MAX_FRAME_SIZE {
            return Err(anyhow::anyhow!("Invalid Input".to_string()));
        }

        let n = self.state.write_message(&item, &mut buf)?;
        dst.reserve(HEADER_LEN + n);
        dst.put_uint(n as u64, HEADER_LEN);
        dst.put_slice(&buf[..n]);

        Ok(())
    }
}

impl Decoder for NoiseCodec {
    type Item = BytesMut;

    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut buf = [0u8; MAX_FRAME_SIZE];

        if src.len() < HEADER_LEN {
            return Ok(None);
        }

        let len = src.get_uint(HEADER_LEN) as usize;

        if src.len() < len {
            return Ok(None);
        }

        let payload = src.split_to(len);
        let n = self.state.read_message(&payload, &mut buf)?;

        Ok(Some(BytesMut::from(&buf[..n])))
    }
}

pub struct Builder {
    params: &'static str,
    initiator: bool,
}

impl Builder {
    fn new(params: &'static str, initiator: bool) -> Self {
        Self { params, initiator }
    }

    fn new_codec(self) -> Result<NoiseCodec> {
        let builder = snow::Builder::new(self.params.parse()?);
        let keypair = builder.generate_keypair()?;
        let builder = builder.local_private_key(&keypair.private);
        let noise = match self.initiator {
            true => builder.build_initiator()?,
            false => builder.build_responder()?,
        };

        Ok(NoiseCodec {
            builder: self,
            state: NoiseState::HandShake(noise),
        })
    }

    pub fn new_framed<T>(self, inner: T) -> Result<Framed<T, NoiseCodec>>
    where
        T: AsyncRead + AsyncWrite,
    {
        let codec = self.new_codec()?;
        Ok(Framed::new(inner, codec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn it_works() -> Result<()> {
        let mut client = NoiseCodec::builder(NOISE_PARAMS, true).new_codec()?;
        let mut server = NoiseCodec::builder(NOISE_PARAMS, false).new_codec()?;

        let mut buf = BytesMut::new();

        client.encode(Bytes::from_static(b"Hello"), &mut buf)?;
        let mut msg = buf.split_to(buf.len());

        let msg = server.decode(&mut msg)?.unwrap();
        server.encode(msg.freeze(), &mut buf).unwrap();
        let mut msg = buf.split_to(buf.len());

        let msg = client.decode(&mut msg)?.unwrap();
        client.encode(msg.freeze(), &mut buf).unwrap();
        let mut msg = buf.split_to(buf.len());

        let msg = server.decode(&mut msg)?.unwrap();
        assert_eq!(msg.freeze().as_ref(), b"Hello");

        client.into_transport_mode()?;
        server.into_transport_mode()?;

        Ok(())
    }
}
