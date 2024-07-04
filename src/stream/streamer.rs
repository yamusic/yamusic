use std::{
    io::{Read, Seek, SeekFrom},
    sync::{Arc, RwLock},
    thread,
};

use flume::{Receiver, Sender};
use tokio_util::bytes::Bytes;

pub struct AudioStreamer {
    url: String,
    client: Arc<reqwest::blocking::Client>,
    ring_buffer: Receiver<u8>,
    position: Arc<RwLock<u64>>,
    eof: bool,
    pub total_bytes: u64,
}

impl AudioStreamer {
    pub fn new(
        url: String,
        // prefetch_bytes: u64,
        fetch_amount: u64,
    ) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::new();
        let total_bytes = Self::fetch_total_bytes(&client, &url)?;
        let (tx, rx) = flume::bounded((fetch_amount * 2) as usize);

        let streamer = Self {
            url,
            client: Arc::new(client),
            ring_buffer: rx,
            position: Arc::new(RwLock::new(0)),
            eof: false,
            total_bytes,
        };

        Self::fetch(
            streamer.url.clone(),
            streamer.client.clone(),
            tx,
            total_bytes,
            fetch_amount,
            streamer.position.clone(),
        );

        Ok(streamer)
    }

    fn fetch(
        url: String,
        client: Arc<reqwest::blocking::Client>,
        ring_buffer: Sender<u8>,
        total_bytes: u64,
        fetch_amount: u64,
        position: Arc<RwLock<u64>>,
    ) {
        thread::spawn(move || {
            let mut current_position = *position.read().unwrap();

            while current_position < total_bytes {
                let end =
                    (current_position + fetch_amount).min(total_bytes - 1);

                if let Ok(bytes) = Self::fetch_range_bytes(
                    &client.clone(),
                    &url,
                    current_position,
                    end,
                ) {
                    for b in bytes {
                        if ring_buffer.send(b).is_err() {
                            break;
                        }
                    }

                    current_position = end + 1;
                }
            }
        });
    }

    fn fetch_range_bytes(
        client: &reqwest::blocking::Client,
        url: &str,
        start: u64,
        end: u64,
    ) -> anyhow::Result<Bytes> {
        Ok(client
            .get(url)
            .header("Range", format!("bytes={}-{}", start, end))
            .send()?
            .bytes()?)
    }

    fn fetch_total_bytes(
        client: &reqwest::blocking::Client,
        url: &str,
    ) -> anyhow::Result<u64> {
        Ok(client
            .head(url)
            .send()?
            .headers()
            .get("Content-Length")
            .unwrap()
            .to_str()?
            .parse()?)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.eof {
            return Ok(0);
        }

        let mut read_bytes = 0;

        while read_bytes < buf.len() {
            if let Ok(b) = self.ring_buffer.recv() {
                buf[read_bytes] = b;
                read_bytes += 1;
            } else {
                self.eof = true;
                break;
            }
        }

        Ok(read_bytes)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(pos) => {
                *self.position.write().unwrap() = pos;
                Ok(pos)
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid seek",
            )),
        }
    }
}

impl Read for AudioStreamer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read(buf)
    }
}

impl Seek for AudioStreamer {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.seek(pos)
    }
}
