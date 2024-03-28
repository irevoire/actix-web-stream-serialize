use std::io::Write;

use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use bytes::Bytes;
use futures_core::Stream;
use serde::{ser::SerializeSeq, Serialize};

pub struct Truc {
    end: usize,
}

impl Serialize for Truc {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.end))?;
        for e in 0..self.end {
            seq.serialize_element(&e)?;
        }
        seq.end()
    }
}

pub struct Writer2Streamer {
    sender: tokio::sync::mpsc::Sender<Result<Bytes, anyhow::Error>>,
}

impl Write for Writer2Streamer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .blocking_send(Ok(buf.to_vec().into()))
            .map_err(std::io::Error::other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn stream(
    data: impl Serialize + Send + Sync + 'static,
) -> impl Stream<Item = Result<Bytes, anyhow::Error>> {
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Bytes, anyhow::Error>>(1);

    tokio::task::spawn_blocking(move || serde_json::to_writer(Writer2Streamer { sender }, &data));
    futures_util::stream::unfold(receiver, |mut receiver| async {
        receiver.recv().await.map(|value| (value, receiver))
    })
}

#[get("/")]
async fn greet() -> impl Responder {
    let truc = Truc { end: 10 };
    let stream = stream(truc);

    HttpResponse::Ok().streaming(stream)
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    println!("starting on http://localhost:4000");

    HttpServer::new(|| {
        App::new()
            .service(greet)
            .wrap(actix_web::middleware::Compress::default())
    })
    .bind(("127.0.0.1", 4000))?
    .run()
    .await
}
