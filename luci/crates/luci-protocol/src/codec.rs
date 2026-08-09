use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use std::io;
use serde::{de::DeserializeOwned, Serialize};

pub struct LengthPrefixedCodec<T> {
    max_size: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T> LengthPrefixedCodec<T> {
    pub fn new() -> Self {
        Self {
            max_size: 10 * 1024 * 1024, // 10MB default
            _marker: std::marker::PhantomData,
        }
    }

    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Default for LengthPrefixedCodec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: DeserializeOwned> Decoder for LengthPrefixedCodec<T> {
    type Item = T;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, io::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let length = (&src[0..4]).get_u32() as usize;

        if length > self.max_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes (max: {})", length, self.max_size),
            ));
        }

        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());
            return Ok(None);
        }

        src.advance(4);
        let data = src.split_to(length);

        serde_json::from_slice(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

impl<T: Serialize> Encoder<T> for LengthPrefixedCodec<T> {
    type Error = io::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), io::Error> {
        let json = serde_json::to_vec(&item)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if json.len() > self.max_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes (max: {})", json.len(), self.max_size),
            ));
        }

        dst.reserve(4 + json.len());
        dst.put_u32(json.len() as u32);
        dst.extend_from_slice(&json);

        Ok(())
    }
}
