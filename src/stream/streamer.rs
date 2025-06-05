use std::io::{Cursor, Read, Seek, SeekFrom};

use flume::{Receiver, Sender};
use tokio_util::bytes::Bytes;

pub struct AudioStreamer {
    cursor: Cursor<Vec<u8>>,
    rx: Option<Receiver<u8>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    pub total_bytes: u64,
}

impl AudioStreamer {
    pub async fn new(
        url: String,
        prefetch_bytes: u64,
        fetch_amount: u64,
    ) -> color_eyre::Result<Self> {
        let client = reqwest::Client::new();
        let total_bytes = Self::fetch_total_bytes(&client, &url).await?;

        let initial_fetch_end =
            prefetch_bytes.min(total_bytes.saturating_sub(1));
        let initial_bytes =
            Self::fetch_range_bytes(&client, &url, 0, initial_fetch_end)
                .await?;

        let buffer = initial_bytes.to_vec();
        let (tx, rx) = flume::bounded((fetch_amount * 2) as usize);

        let handle = if initial_fetch_end + 1 < total_bytes {
            Some(tokio::spawn(Self::fetch_remaining(
                client,
                url,
                tx,
                initial_fetch_end + 1,
                total_bytes,
                fetch_amount,
            )))
        } else {
            None
        };

        Ok(Self {
            cursor: Cursor::new(buffer),
            rx: Some(rx),
            handle,
            total_bytes,
        })
    }

    async fn fetch_remaining(
        client: reqwest::Client,
        url: String,
        sender: Sender<u8>,
        mut start: u64,
        total_bytes: u64,
        fetch_amount: u64,
    ) {
        while start < total_bytes {
            let end = (start + fetch_amount).min(total_bytes - 1);

            if let Ok(bytes) =
                Self::fetch_range_bytes(&client, &url, start, end).await
            {
                for byte in bytes {
                    if sender.send(byte).is_err() {
                        return;
                    }
                }
                start = end + 1;
            } else {
                break;
            }
        }
    }

    async fn fetch_range_bytes(
        client: &reqwest::Client,
        url: &str,
        start: u64,
        end: u64,
    ) -> color_eyre::Result<Bytes> {
        Ok(client
            .get(url)
            .header("Range", format!("bytes={start}-{end}"))
            .send()
            .await?
            .bytes()
            .await?)
    }

    async fn fetch_total_bytes(
        client: &reqwest::Client,
        url: &str,
    ) -> color_eyre::Result<u64> {
        Ok(client
            .head(url)
            .send()
            .await?
            .headers()
            .get("Content-Length")
            .ok_or_else(|| color_eyre::eyre::eyre!("No content length found"))?
            .to_str()?
            .parse()?)
    }

    fn ensure_buffer(&mut self, needed_bytes: usize) {
        if let Some(receiver) = &self.rx {
            let current_len = self.cursor.get_ref().len();
            let current_pos = self.cursor.position() as usize;

            if current_pos + needed_bytes > current_len {
                let mut buffer = self.cursor.get_ref().clone();
                let mut received_any = true;

                while received_any && buffer.len() < current_pos + needed_bytes
                {
                    received_any = false;

                    for _ in 0..1024 {
                        match receiver.try_recv() {
                            Ok(byte) => {
                                buffer.push(byte);
                                received_any = true;
                            }
                            Err(_) => break,
                        }
                    }
                }

                let pos = self.cursor.position();
                self.cursor = Cursor::new(buffer);
                self.cursor.set_position(pos);
            }
        }
    }

    fn ensure_buffer_for_position(
        &mut self,
        target_pos: u64,
    ) -> std::io::Result<()> {
        if target_pos > self.total_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position beyond file size",
            ));
        }

        if let Some(receiver) = &self.rx {
            let target_pos_usize = target_pos as usize;
            let mut buffer = self.cursor.get_ref().clone();

            while buffer.len() <= target_pos_usize {
                match receiver.recv() {
                    Ok(byte) => buffer.push(byte),
                    Err(_) => break,
                }
            }

            let current_pos = self.cursor.position();
            self.cursor = Cursor::new(buffer);
            self.cursor.set_position(current_pos);

            if target_pos_usize >= self.cursor.get_ref().len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Seek position beyond available data",
                ));
            }
        }

        Ok(())
    }
}

impl Read for AudioStreamer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.ensure_buffer(buf.len());
        self.cursor.read(buf)
    }
}

impl Seek for AudioStreamer {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let current_pos = self.cursor.position();
        let target_pos = match pos {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(offset) => {
                if offset >= 0 {
                    self.total_bytes.saturating_add(offset as u64)
                } else {
                    self.total_bytes.saturating_sub((-offset) as u64)
                }
            }
            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    current_pos.saturating_add(offset as u64)
                } else {
                    current_pos.saturating_sub((-offset) as u64)
                }
            }
        };

        self.ensure_buffer_for_position(target_pos)?;
        self.cursor.seek(pos)
    }
}

impl Drop for AudioStreamer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
