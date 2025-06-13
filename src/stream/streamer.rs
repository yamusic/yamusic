use crate::audio::progress::TrackProgress;
use color_eyre::{Result, eyre::eyre};
use flume::{Receiver, Sender};
use reqwest::Client;
use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::block_in_place;

const BUFFER_SIZE: usize = 4 * 1024 * 1024;
const PREFETCH_SIZE: usize = 512 * 1024;
const PREFETCH_TRIGGER: usize = 128 * 1024;
const MIN_INITIAL_DATA: usize = 8 * 1024;
const MAX_ATTEMPTS: usize = 100;

#[derive(Debug)]
struct BufferState {
    data: VecDeque<u8>,
    start_pos: u64,
    total_bytes: u64,
    eof: bool,
    pending: Option<(u64, u64)>,
}

impl BufferState {
    fn new(total_bytes: u64) -> Self {
        Self {
            data: VecDeque::with_capacity(BUFFER_SIZE),
            start_pos: 0,
            total_bytes,
            eof: false,
            pending: None,
        }
    }

    fn contains(&self, pos: u64) -> bool {
        pos >= self.start_pos && pos < self.start_pos + self.data.len() as u64
    }

    fn available_from(&self, pos: u64) -> usize {
        if !self.contains(pos) {
            return 0;
        }
        let off = (pos - self.start_pos) as usize;
        self.data.len() - off
    }

    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> usize {
        let avail = self.available_from(pos);
        let len = buf.len().min(avail);
        let off = (pos - self.start_pos) as usize;
        buf[..len].copy_from_slice(&self.data.as_slices().0[off..off + len]);
        len
    }

    fn append(&mut self, new: &[u8], start: u64) {
        if let Some((s, e)) = self.pending
            && (s..e).contains(&start)
        {
            self.pending = None;
        }
        let exp_end = self.start_pos + self.data.len() as u64;
        if start != exp_end && !self.data.is_empty() {
            self.data.clear();
            self.start_pos = start;
        }
        while self.data.len() + new.len() > BUFFER_SIZE {
            self.data.pop_front();
            self.start_pos += 1;
        }
        if start + new.len() as u64 >= self.total_bytes {
            self.eof = true;
        }
        self.data.extend(new);
    }

    fn clear(&mut self, start: u64) {
        self.data.clear();
        self.start_pos = start;
        self.pending = None;
    }

    fn end_pos(&self) -> u64 {
        self.start_pos + self.data.len() as u64
    }

    fn should_prefetch(&self, pos: u64) -> bool {
        !self.eof && self.pending.is_none() && self.available_from(pos) < PREFETCH_TRIGGER
    }

    fn mark_pending(&mut self, start: u64, end: u64) {
        self.pending = Some((start, end));
    }
}

enum FetchCommand {
    Fetch { start: u64, end: u64 },
    Cancel,
    Shutdown,
}

pub struct StreamingDataSource {
    total_bytes: u64,
    buffer: Arc<RwLock<BufferState>>,
    position: Arc<Mutex<u64>>,
    fetch_tx: Sender<FetchCommand>,
    fetch_rx: Receiver<()>,
    _handle: tokio::task::JoinHandle<()>,
    rt: tokio::runtime::Handle,
}

impl StreamingDataSource {
    pub async fn new(url: String, progress: Arc<TrackProgress>) -> Result<Self> {
        let client = Client::new();
        let resp = client.head(&url).send().await?;
        let total = resp
            .headers()
            .get("content-length")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre!("content-length missing"))?;

        progress.set_total_bytes(total);

        let buffer = Arc::new(RwLock::new(BufferState::new(total)));
        let position = Arc::new(Mutex::new(0));
        let (tx_cmd, rx_cmd) = flume::unbounded();
        let (tx_res, rx_res) = flume::unbounded();

        let h = {
            let client = client.clone();
            let u = url.clone();
            let buf = Arc::clone(&buffer);
            let prog = Arc::clone(&progress);
            tokio::spawn(async move {
                Self::fetch_loop(client, u, buf, prog, rx_cmd, tx_res).await;
            })
        };

        let src = Self {
            total_bytes: total,
            buffer,
            position,
            fetch_tx: tx_cmd,
            fetch_rx: rx_res,
            _handle: h,
            rt: tokio::runtime::Handle::current(),
        };

        src.fetch(0, PREFETCH_SIZE as u64).await?;
        src.wait_for(0, MIN_INITIAL_DATA).await?;
        Ok(src)
    }

    pub fn get_total_bytes(&self) -> u64 {
        self.total_bytes
    }

    async fn fetch_loop(
        client: Client,
        url: String,
        buffer: Arc<RwLock<BufferState>>,
        prog: Arc<TrackProgress>,
        rx_cmd: Receiver<FetchCommand>,
        tx_res: Sender<()>,
    ) {
        let mut current: Option<tokio::task::JoinHandle<()>> = None;
        while let Ok(cmd) = rx_cmd.recv_async().await {
            match cmd {
                FetchCommand::Fetch { start, end } => {
                    if let Some(t) = current.take() {
                        t.abort();
                    }
                    buffer.write().await.mark_pending(start, end);
                    let c = client.clone();
                    let u = url.clone();
                    let buf = Arc::clone(&buffer);
                    let pr = Arc::clone(&prog);
                    let tx = tx_res.clone();
                    current = Some(tokio::spawn(async move {
                        if let Ok(data) = StreamingDataSource::fetch_range(&c, &u, start, end).await
                        {
                            let end_pos = {
                                let mut bb = buf.write().await;
                                bb.append(&data, start);
                                bb.end_pos()
                            };
                            pr.set_buffered_bytes(end_pos);
                            let _ = tx.send_async(()).await;
                        }
                    }));
                }
                FetchCommand::Cancel => {
                    if let Some(t) = current.take() {
                        t.abort();
                    }
                    buffer.write().await.pending = None;
                }
                FetchCommand::Shutdown => {
                    if let Some(t) = current.take() {
                        t.abort();
                    }
                    break;
                }
            }
        }
    }

    async fn fetch_range(client: &Client, url: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let hdr = format!("bytes={}-{}", start, end - 1);
        let resp = client.get(url).header("Range", hdr).send().await?;
        Ok(resp.bytes().await?.to_vec())
    }

    async fn fetch(&self, start: u64, size: u64) -> Result<()> {
        let end = (start + size).min(self.total_bytes);
        self.fetch_tx
            .send_async(FetchCommand::Fetch { start, end })
            .await
            .map_err(|_| eyre!("fetch cmd failed"))
    }

    async fn wait_for(&self, pos: u64, min: usize) -> Result<()> {
        if self.buffer.read().await.available_from(pos) >= min || self.buffer.read().await.eof {
            return Ok(());
        }

        let mut attempts = 0;
        while attempts < MAX_ATTEMPTS {
            if let Ok(Ok(_)) = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                self.fetch_rx.recv_async(),
            )
            .await
            {
                if self.buffer.read().await.available_from(pos) >= min
                    || self.buffer.read().await.eof
                {
                    return Ok(());
                }
            } else {
                attempts += 1;
            }
        }
        Err(eyre!("wait_for_data timed out"))
    }

    async fn ensure(&self, pos: u64) -> Result<()> {
        if !self.buffer.read().await.contains(pos) {
            let _ = self.fetch_tx.send_async(FetchCommand::Cancel).await;
            self.buffer.write().await.clear(pos);
            let size = PREFETCH_SIZE.min((self.total_bytes - pos) as usize) as u64;
            self.fetch(pos, size).await?;
            self.wait_for(pos, MIN_INITIAL_DATA.min(size as usize))
                .await?;
        }
        Ok(())
    }

    fn trigger_prefetch(&self) {
        let should = block_in_place(|| {
            self.rt.block_on(async {
                let p = *self.position.lock().await;
                self.buffer.read().await.should_prefetch(p)
            })
        });
        if should {
            let (start, size) = block_in_place(|| {
                self.rt.block_on(async {
                    let buf = self.buffer.read().await;
                    (
                        buf.end_pos(),
                        PREFETCH_SIZE.min((self.total_bytes - buf.end_pos()) as usize),
                    )
                })
            });
            if size > 0 {
                let _ = self.fetch_tx.try_send(FetchCommand::Fetch {
                    start,
                    end: start + size as u64,
                });
            }
        }
    }
}

impl Read for StreamingDataSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let pos = block_in_place(|| self.rt.block_on(async { *self.position.lock().await }));
        if pos >= self.total_bytes {
            return Ok(0);
        }

        let has = block_in_place(|| {
            self.rt
                .block_on(async { self.buffer.read().await.contains(pos) })
        });
        if !has {
            block_in_place(|| self.rt.block_on(async { self.ensure(pos).await }))
                .map_err(std::io::Error::other)?;
        }

        let bytes = block_in_place(|| {
            self.rt.block_on(async {
                let mut bb = self.buffer.write().await;
                bb.read_at(pos, buf)
            })
        });
        if bytes > 0 {
            block_in_place(|| {
                self.rt
                    .block_on(async { *self.position.lock().await += bytes as u64 })
            });
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
                    self.total_bytes + off as u64
                } else {
                    self.total_bytes.saturating_sub((-off) as u64)
                }
            }
            SeekFrom::Current(off) => {
                let cur =
                    block_in_place(|| self.rt.block_on(async { *self.position.lock().await }));
                if off >= 0 {
                    cur + off as u64
                } else {
                    cur.saturating_sub((-off) as u64)
                }
            }
        }
        .min(self.total_bytes);

        block_in_place(|| {
            self.rt
                .block_on(async { *self.position.lock().await = new })
        });
        block_in_place(|| self.rt.block_on(async { self.ensure(new).await }))
            .map_err(std::io::Error::other)?;
        Ok(new)
    }
}

impl Drop for StreamingDataSource {
    fn drop(&mut self) {
        let _ = self.fetch_tx.try_send(FetchCommand::Shutdown);
    }
}
