use std::{
    fs::File,
    io::{self, Read},
    io::{Seek, Write},
};

use crate::utils::{ceil_div, SECTOR_SIZE};

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
        let size = ceil_div(size, SECTOR_SIZE as u64) * SECTOR_SIZE as u64;
        self.file.set_len(size)?;
        Ok(())
    }
    fn is_file_type(&self) -> io::Result<bool> {
        Ok(true)
    }
    fn seek(&mut self, position: usize) -> io::Result<()> {
        self.position = position;
        self.file
            .seek(std::io::SeekFrom::Start(self.position as u64))?;
        Ok(())
    }
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
    fn write(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.write(buf)
    }
    fn total_size(&self) -> usize {
        self.file.metadata().unwrap().len() as usize
    }
    fn chs_info(&self) -> (u16, u8, u8) {
        (self.total_size() as u16 / 255 / 63, 255, 63)
    }
}
