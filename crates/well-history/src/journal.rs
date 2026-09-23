use memmap2::MmapMut;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EventRecord {
    StdinEvent(Vec<u8>),
    StdoutEvent(Vec<u8>),
    GitDigest(String),
}

/// Append-only ring buffer backed by memmap2.
pub struct Journal {
    _file: File,
    mmap: MmapMut,
    offset: usize,
}

impl Journal {
    pub fn new(path: &str, size: usize) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.set_len(size as u64)?;

        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(Self {
            _file: file,
            mmap,
            offset: 0,
        })
    }

    pub fn append(&mut self, event: &EventRecord) -> io::Result<()> {
        let encoded = bincode::serialize(event)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let len = encoded.len();

        // Very basic bounds check (not a true ring buffer yet, just append)
        if self.offset + len > self.mmap.len() {
            return Err(io::Error::other("Journal full"));
        }

        self.mmap[self.offset..self.offset + len].copy_from_slice(&encoded);
        self.offset += len;

        Ok(())
    }

    pub fn flush_zstd(&self, out_path: &str) -> io::Result<()> {
        let out_file = File::create(out_path)?;
        let mut encoder = zstd::Encoder::new(out_file, 0)?;
        encoder.write_all(&self.mmap[0..self.offset])?;
        encoder.finish()?;
        Ok(())
    }
}
