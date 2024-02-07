use std::{
    fs::File,
    io::{self, Read},
    io::{Seek, Write},
};

use super::FileHandler;

pub struct RawType {
    file: File,
    _mode: super::FileOpsMode,
    position: usize,
}

impl RawType {
    pub fn new(file: File, mode: super::FileOpsMode) -> Self {
        RawType {
            file: file,
            _mode: mode,
            position: 0,
        }
    }
}

impl FileHandler for RawType {
    fn create(&mut self, size: u64) -> io::Result<()> {
        self.file.set_len(size)?;
        Ok(())
    }
    fn is_file_type(&self) -> io::Result<bool> {
        Ok(true)
    }
    fn seek(&mut self, position: usize) {
        self.position = position;
        self.file
            .seek(std::io::SeekFrom::Start(self.position as u64))
            .unwrap();
    }
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
    fn write(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.write(buf)
    }
}
