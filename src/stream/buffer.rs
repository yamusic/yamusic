use std::collections::VecDeque;

pub(crate) const BUFFER_SIZE: usize = 16 * 1024 * 1024;
pub(crate) const PREFETCH_TRIGGER: usize = 256 * 1024;

#[derive(Debug)]
pub struct BufferState {
    data: VecDeque<u8>,
    start_pos: u64,
    total_bytes: u64,
    pub(crate) eof: bool,
    pending: Option<(u64, u64)>,
    max_buffered_from_start: u64,
    buffering_base: u64,
}

impl BufferState {
    pub fn new(total_bytes: u64) -> Self {
        Self {
            data: VecDeque::with_capacity(BUFFER_SIZE),
            start_pos: 0,
            total_bytes,
            eof: false,
            pending: None,
            max_buffered_from_start: 0,
            buffering_base: 0,
        }
    }

    pub fn contains(&self, pos: u64) -> bool {
        pos >= self.start_pos && pos < self.start_pos + self.data.len() as u64
    }

    pub fn available_from(&self, pos: u64) -> usize {
        if !self.contains(pos) {
            return 0;
        }
        let off = (pos - self.start_pos) as usize;
        self.data.len() - off
    }

    pub fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> usize {
        let avail = self.available_from(pos);
        let len = buf.len().min(avail);
        let off = (pos - self.start_pos) as usize;

        let (s1, s2) = self.data.as_slices();
        if off + len <= s1.len() {
            buf[..len].copy_from_slice(&s1[off..off + len]);
        } else {
            let mut copied = 0;
            if off < s1.len() {
                let a = &s1[off..];
                buf[..a.len()].copy_from_slice(a);
                copied += a.len();
            }
            if copied < len {
                let need = len - copied;
                buf[copied..copied + need].copy_from_slice(&s2[..need]);
            }
        }
        len
    }

    pub fn append(&mut self, new: &[u8], start: u64) -> bool {
        if new.is_empty() {
            return false;
        }

        if let Some((s, e)) = self.pending {
            if start >= s && start < e {
                self.pending = None;
            }
        }

        if start < self.start_pos {
            return false;
        }

        if self.data.is_empty() {
            if start != self.start_pos {
                return false;
            }
        } else {
            let exp_end = self.start_pos + self.data.len() as u64;
            if start != exp_end {
                self.data.clear();
                self.start_pos = start;
                self.eof = false;
            }
        }

        let overflow = (self.data.len() + new.len()).saturating_sub(BUFFER_SIZE);
        if overflow > 0 {
            self.data.drain(..overflow);
            self.start_pos += overflow as u64;
        }

        if start + new.len() as u64 >= self.total_bytes {
            self.eof = true;
        }

        self.data.extend(new);

        let new_end = self.start_pos + self.data.len() as u64;
        if self.start_pos >= self.buffering_base
            && self.start_pos <= self.max_buffered_from_start + 1
        {
            self.max_buffered_from_start = self.max_buffered_from_start.max(new_end);
        }

        true
    }

    pub fn clear(&mut self, start: u64) {
        self.data.clear();
        self.start_pos = start;
        self.pending = None;
        self.eof = false;
        if start <= self.max_buffered_from_start {
            self.buffering_base = start;
            self.max_buffered_from_start = start;
        }
    }

    pub fn discard_before(&mut self, pos: u64) {
        if pos <= self.start_pos {
            return;
        }
        let drop = ((pos - self.start_pos) as usize).min(self.data.len());
        if drop == 0 {
            return;
        }
        self.data.drain(..drop);
        self.start_pos += drop as u64;

        let current_end = self.start_pos + self.data.len() as u64;
        self.max_buffered_from_start = self.max_buffered_from_start.max(current_end);
    }

    pub fn end_pos(&self) -> u64 {
        self.start_pos + self.data.len() as u64
    }

    pub fn max_buffered_from_start(&self) -> u64 {
        self.max_buffered_from_start
    }

    pub fn should_prefetch(&self, pos: u64) -> bool {
        !self.eof && self.pending.is_none() && self.available_from(pos) < PREFETCH_TRIGGER
    }

    pub fn mark_pending(&mut self, start: u64, end: u64) {
        self.pending = Some((start, end));
    }

    pub fn clear_pending(&mut self) {
        self.pending = None;
    }
}
