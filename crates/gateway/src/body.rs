use std::io;

use bytes::Bytes;
use futures_util::{Stream, TryStreamExt};
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Empty, Full, StreamBody};
use hyper::body::Frame;

pub type Body = UnsyncBoxBody<Bytes, io::Error>;

pub fn empty() -> Body {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed_unsync()
}

pub fn full(b: impl Into<Bytes>) -> Body {
    Full::new(b.into())
        .map_err(|never| match never {})
        .boxed_unsync()
}

pub fn stream<S>(s: S) -> Body
where
    S: Stream<Item = io::Result<Bytes>> + Send + 'static,
{
    StreamBody::new(s.map_ok(Frame::data)).boxed_unsync()
}

/// Corpo alimentato da un task (download dallo storage + scrittura in cache).
pub fn channel(rx: tokio::sync::mpsc::Receiver<io::Result<Bytes>>) -> Body {
    stream(futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }))
}
