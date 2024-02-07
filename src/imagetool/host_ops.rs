use std::{
    fs::File,
    io::{self},
};

use self::raw::RawType;

pub mod raw;

pub enum FileOpsMode {
    ReadOnly,
    ReadWrite,
}

pub trait FileHandler {
    fn is_file_type(&self) -> io::Result<bool>;
    fn create(&mut self, size: u64) -> io::Result<()>;
    fn seek(&mut self, position: usize);
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

pub fn new(file: File, mode: FileOpsMode) -> io::Result<Box<dyn FileHandler>> {
    let file_types: Vec<Box<dyn FileHandler>> =
        vec![Box::new(RawType::new(file.try_clone()?, mode))];

    for file_type in file_types {
        if file_type.is_file_type()? {
            return Ok(file_type);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::Other,
        "Unsupported file type",
    ))
}
