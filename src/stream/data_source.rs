use crate::audio::progress::TrackProgress;
use color_eyre::{Result, eyre::eyre};
use flume::{Receiver, Sender};
use reqwest::blocking::Client;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

use super::buffer::BufferState;

const PREFETCH_SIZE: usize = 256 * 1024;
const MIN_INITIAL_DATA: usize = 64 * 1024;
const MAX_ATTEMPTS: usize = 100;

enum FetchCommand {
    Fetch {
        start: u64,
        end: u64,
        generation: u64,
    },
    Shutdown,
}

pub struct StreamingDataSource {
    total_bytes: u64,
    buffer: Arc<Mutex<BufferState>>,
    position: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
    fetch_tx: Sender<FetchCommand>,
    fetch_rx: Receiver<()>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl StreamingDataSource {
    pub fn new(client: Client, url: String, progress: Arc<TrackProgress>) -> Result<Self> {
        let head = client.head(&url).send()?;
        let total = head
            .headers()
            .get("content-length")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre!("content-length missing"))?;

        progress.set_total_bytes(total);

        let buffer = Arc::new(Mutex::new(BufferState::new(total)));
        let position = Arc::new(AtomicU64::new(0));
        let (tx_cmd, rx_cmd) = flume::unbounded();
        let (tx_res, rx_res) = flume::unbounded();
        let generation = Arc::new(AtomicU64::new(0));

        let buffer_clone = Arc::clone(&buffer);
        let url_clone = url.clone();
        let progress_clone = Arc::clone(&progress);
        let tx_res_clone = tx_res.clone();
        let generation_clone = Arc::clone(&generation);

        let thread_handle = {
            thread::spawn(move || {
                Self::fetch_loop_blocking(
                    client,
                    url_clone,
                    buffer_clone,
                    progress_clone,
                    generation_clone,
                    rx_cmd,
                    tx_res_clone,
                );
            })
        };

        let src = Self {
            total_bytes: total,
            buffer,
            position,
            generation,
            fetch_tx: tx_cmd,
            fetch_rx: rx_res,
            thread_handle: Some(thread_handle),
        };

        src.fetch(0, PREFETCH_SIZE as u64)?;
        src.wait_for(0, MIN_INITIAL_DATA)?;
        Ok(src)
    }

    fn fetch_loop_blocking(
        client: Client,
        url: String,
        buffer: Arc<Mutex<BufferState>>,
        prog: Arc<TrackProgress>,
        generation: Arc<AtomicU64>,
        rx_cmd: Receiver<FetchCommand>,
        tx_res: Sender<()>,
    ) {
        while let Ok(cmd) = rx_cmd.recv() {
            match cmd {
                FetchCommand::Fetch {
                    start,
                    end,
                    generation: request_generation,
                } => {
                    {
                        let mut buf = buffer.lock().unwrap();
                        buf.mark_pending(start, end);
                    }
                    match Self::fetch_range_blocking(&client, &url, start, end) {
                        Ok(data) => {
                            if request_generation != generation.load(Ordering::SeqCst) {
                                let _ = tx_res.send(());
                                continue;
                            }

                            let maybe_end = {
                                let mut buf = buffer.lock().unwrap();
                                if buf.append(&data, start) {
                                    Some(buf.end_pos())
                                } else {
                                    None
                                }
                            };

                            if let Some(end_pos) = maybe_end {
                                prog.set_buffered_bytes(end_pos);
                            }
                            let _ = tx_res.send(());
                        }
                        Err(err) => {
                            let mut buf = buffer.lock().unwrap();
                            buf.clear_pending();
                            let _ = tx_res.send(());
                            eprintln!("fetch_range_blocking error: {:?}", err);
                        }
                    }
                }
                FetchCommand::Shutdown => {
                    break;
                }
            }
        }
    }

    fn fetch_range_blocking(client: &Client, url: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let hdr = format!("bytes={}-{}", start, end.saturating_sub(1));
        let resp = client.get(url).header("Range", hdr).send()?;
        Ok(resp.bytes()?.to_vec())
    }

    fn fetch(&self, start: u64, size: u64) -> Result<()> {
        let end = (start + size).min(self.total_bytes);
        let generation = self.generation.load(Ordering::SeqCst);
        self.fetch_tx
            .send(FetchCommand::Fetch {
                start,
                end,
                generation,
            })
            .map_err(|_| eyre!("fetch cmd failed"))
    }

    fn wait_for(&self, pos: u64, min: usize) -> Result<()> {
        {
            let buf = self.buffer.lock().unwrap();
            if buf.available_from(pos) >= min || buf.eof {
                return Ok(());
            }
        }

        let mut attempts = 0usize;
        while attempts < MAX_ATTEMPTS {
            match self.fetch_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(_) => {
                    let buf = self.buffer.lock().unwrap();
                    if buf.available_from(pos) >= min || buf.eof {
                        return Ok(());
                    }
                }
                Err(flume::RecvTimeoutError::Timeout) => {
                    attempts += 1;
                }
                Err(_) => {
                    break;
                }
            }
        }

        Err(eyre!("wait_for_data timed out"))
    }

    fn ensure(&self, pos: u64) -> Result<()> {
        {
            let buf = self.buffer.lock().unwrap();
            if buf.contains(pos) {
                return Ok(());
            }
        }

        let _ = self.generation.fetch_add(1, Ordering::SeqCst);
        {
            let mut buf = self.buffer.lock().unwrap();
            buf.clear(pos);
        }

        let size = PREFETCH_SIZE.min((self.total_bytes.saturating_sub(pos)) as usize) as u64;
        if size > 0 {
            self.fetch(pos, size)?;
            self.wait_for(pos, PREFETCH_SIZE.min(size as usize))?;
        }

        Ok(())
    }

    fn trigger_prefetch(&self) {
        let (should, start, size) = {
            let pos = self.position.load(Ordering::SeqCst);
            let buf = self.buffer.lock().unwrap();
            if buf.should_prefetch(pos) {
                let start = buf.end_pos();
                let size = PREFETCH_SIZE.min((self.total_bytes.saturating_sub(start)) as usize);
                (size > 0, start, size)
            } else {
                (false, 0, 0)
            }
        };
        if should {
            let generation = self.generation.load(Ordering::SeqCst);
            let _ = self.fetch_tx.try_send(FetchCommand::Fetch {
                start,
                end: start + size as u64,
                generation,
            });
        }
    }

    pub fn get_total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

impl Read for StreamingDataSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let pos = self.position.load(Ordering::SeqCst);
        if pos >= self.total_bytes {
            return Ok(0);
        }

        let has = {
            let b = self.buffer.lock().unwrap();
            b.contains(pos)
        };

        if !has {
            self.ensure(pos).map_err(std::io::Error::other)?;
        }

        let bytes = {
            let mut b = self.buffer.lock().unwrap();
            let read = b.read_at(pos, buf);
            if read > 0 {
                b.discard_before(pos.saturating_add(read as u64));
            }
            read
        };

        if bytes > 0 {
            self.position.fetch_add(bytes as u64, Ordering::SeqCst);
            self.trigger_prefetch();
        }

        Ok(bytes)
    }
}

impl Seek for StreamingDataSource {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let new = match from {
            SeekFrom::Start(o) => o,
            SeekFrom::End(off) => {
                if off >= 0 {
                    self.total_bytes.saturating_add(off as u64)
                } else {
                    self.total_bytes.saturating_sub((-off) as u64)
                }
            }
            SeekFrom::Current(off) => {
                let cur = self.position.load(Ordering::SeqCst);
                if off >= 0 {
                    cur.saturating_add(off as u64)
                } else {
                    cur.saturating_sub((-off) as u64)
                }
            }
        }
        .min(self.total_bytes);

        self.position.store(new, Ordering::SeqCst);
        self.ensure(new).map_err(std::io::Error::other)?;
        Ok(new)
    }
}

impl Drop for StreamingDataSource {
    fn drop(&mut self) {
        let _ = self.fetch_tx.send(FetchCommand::Shutdown);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}
