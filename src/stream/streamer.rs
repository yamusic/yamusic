use std::{
    collections::VecDeque,
    io::{Read, Seek, SeekFrom},
    sync::{Arc, RwLock},
};

use symphonia::core::io::MediaSource;

pub struct AudioStreamer {
    url: String,
    client: reqwest::blocking::Client,
    ring_buffer: Arc<RwLock<VecDeque<u8>>>,
    total_bytes: u64,
    fetch_amount: u64,
    position: u64,
    eof: bool,
}

impl AudioStreamer {
    pub fn new(
        url: String,
        prefetch_bytes: u64,
        fetch_amount: u64,
    ) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::new();
        let total_bytes = Self::fetch_total_bytes(&client, &url)?;

        let mut streamer = Self {
            url,
            client,
            ring_buffer: Arc::new(RwLock::new(VecDeque::new())),
            total_bytes,
            fetch_amount,
            position: 0,
            eof: false,
        };

        streamer.fetch(prefetch_bytes)?;

        Ok(streamer)
    }

    pub fn fetch(&mut self, fetch_amount: u64) -> anyhow::Result<usize> {
        if self.eof || self.position >= self.total_bytes {
            self.eof = true;

            return Ok(0);
        }

        let end = (self.position + fetch_amount).min(self.total_bytes - 1);
        let bytes = self.fetch_range_bytes(self.position, end)?;
        let bytes_read = bytes.len();

        {
            let mut ring_buffer = self
                .ring_buffer
                .write()
                .map_err(|e| anyhow::Error::msg(e.to_string()))?;

            ring_buffer.extend(bytes);
        }
        self.position = end + 1;

        Ok(bytes_read)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        if self.eof {
            return Ok(0);
        }

        let buffer_len = buf.len() as u64;
        let ring_buffer_len;

        {
            let ring_buffer = self
                .ring_buffer
                .read()
                .or_else(|e| anyhow::bail!(e.to_string()))?;
            ring_buffer_len = ring_buffer.len() as u64;
        }

        if buffer_len > ring_buffer_len {
            let fetch_amount =
                std::cmp::max(buffer_len - ring_buffer_len, self.fetch_amount);

            self.fetch(fetch_amount)?;
        }

        let mut ring_buffer = self
            .ring_buffer
            .write()
            .or_else(|e| anyhow::bail!(e.to_string()))?;
        let read_bytes = ring_buffer
            .iter()
            .take(buf.len())
            .enumerate()
            .map(|(i, &b)| {
                buf[i] = b;
                1
            })
            .sum();

        ring_buffer.drain(..read_bytes);

        Ok(read_bytes)
    }

    pub fn seek(&mut self, pos: u64) -> std::io::Result<()> {
        if pos > self.total_bytes {
            self.ring_buffer
                .write()
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    )
                })?
                .clear();
            self.position = self.total_bytes;
            self.eof = true;
            return Ok(());
        }
        self.ring_buffer
            .write()
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?
            .drain(..(pos as usize));
        self.position = pos;

        Ok(())
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
            .ok_or(anyhow::Error::msg("No content length"))?
            .to_str()?
            .parse::<u64>()?)
    }

    fn fetch_range_bytes(
        &self,
        start: u64,
        end: u64,
    ) -> anyhow::Result<Vec<u8>> {
        let range_header = format!("bytes={}-{}", start, end);
        let response = self
            .client
            .get(&self.url)
            .header("Range", range_header)
            .send()?
            .bytes()?;

        Ok(response.to_vec())
    }
}

impl Read for AudioStreamer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read(buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl Seek for AudioStreamer {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(pos) => {
                self.seek(pos)?;
                Ok(self.position)
            }
            SeekFrom::Current(offset) => {
                self.seek(self.position + offset as u64)?;
                Ok(self.position)
            }
            SeekFrom::End(offset) => {
                self.seek(self.total_bytes + offset as u64)?;
                Ok(self.position)
            }
        }
    }
}

impl MediaSource for AudioStreamer {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.total_bytes)
    }
}
