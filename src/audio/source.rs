use std::io::{Read, Seek, SeekFrom};

use stream_download::{storage::temp::TempStorageProvider, StreamDownload};
use symphonia::core::io::MediaSource;

pub struct MediaSourceWrapper(pub StreamDownload<TempStorageProvider>);

impl Seek for MediaSourceWrapper {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl Read for MediaSourceWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl MediaSource for MediaSourceWrapper {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}
