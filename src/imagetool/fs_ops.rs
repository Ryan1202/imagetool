use std::fmt::Debug;
use std::io;

use chrono::{NaiveDate, NaiveTime};

use crate::host_ops::FileHandler;
use crate::vfs::FileType;
use crate::vfs::PtPosition;

use self::fat::FatFs;

pub mod fat;

pub struct Request {
    pub idx: usize,
    pub offset: usize,
}

pub trait FileSystem {
    fn init(&mut self, disk: &mut Box<dyn FileHandler>, pos: &PtPosition) -> io::Result<()>;
    fn open(&mut self, disk: &mut Box<dyn FileHandler>, path: String) -> io::Result<Request>;
    fn read(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        req: &mut Request,
        buf: &mut [u8],
        size: usize,
    ) -> io::Result<usize>;
    fn write(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        req: &mut Request,
        buf: &mut [u8],
        size: usize,
    ) -> io::Result<usize>;
    fn create_file(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        path: &String,
        ftype: FileType,
        attr: u16,
        create_date: &NaiveDate,
        create_time: &NaiveTime,
        write_date: &NaiveDate,
        write_time: &NaiveTime,
        last_acc_date: &NaiveDate,
        file_size: u32,
    ) -> io::Result<Request>;
    fn delete_file(&mut self, disk: &mut Box<dyn FileHandler>, req: &mut Request)
        -> io::Result<()>;
}

impl Debug for dyn FileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileSystem {{ ... }}")
    }
}

pub fn new(
    disk: &mut Box<dyn FileHandler>,
    pos: &PtPosition,
    id: u8,
) -> io::Result<Option<Box<dyn FileSystem>>> {
    let mut fs: Box<dyn FileSystem>;
    match id {
        0x01 | 0x04 | 0x06 | 0x0b | 0x0c | 0x0e => {
            fs = Box::new(FatFs::new_empty());
        }
        _ => {
            return Ok(None);
        }
    }

    fs.init(disk, pos)?;
    Ok(Some(fs))
}
