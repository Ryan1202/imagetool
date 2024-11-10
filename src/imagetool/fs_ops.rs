use std::fmt::Debug;
use std::io;
use std::path::Components;
use std::sync::Arc;

use chrono::{NaiveDate, NaiveTime};

use crate::host_ops::FileHandler;
use crate::vfs::FileType;
use crate::vfs::PtPosition;

use self::fat::FatFs;

use super::vfs::FileNode;

pub mod fat;

pub struct Request {
    pub idx: usize,
    pub offset: usize,
}

pub trait FileSystem: Send + Sync {
    fn init(&mut self, disk: &mut Box<dyn FileHandler>, pos: &PtPosition) -> io::Result<()>;

}

pub trait FileOps: Send + Sync + Debug {
    
    fn open(&mut self, disk: &mut Box<dyn FileHandler>, fs_root: Arc<FileNode>, path: Components) -> io::Result<Arc<FileNode>>;
    
    

    fn create_file(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        fs_root: Arc<FileNode>,
        path: Components,
        is_directory: bool,
        permission: u16,
        create_date: &NaiveDate,
        create_time: &NaiveTime,
        write_date: &NaiveDate,
        write_time: &NaiveTime,
        last_acc_date: &NaiveDate,
        file_size: u32,
    ) -> io::Result<Arc<FileNode>>;
    fn delete_file(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        fs_root: Arc<FileNode>,
        path: Components,
    ) -> io::Result<()>;

    fn read(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        size: usize,
        buf: &mut [u8],
    ) -> io::Result<usize>;
    fn write(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        size: usize,
        buf: &mut [u8],
    ) -> io::Result<usize>;
}

pub fn fs_init(
    name: String,
    disk: &mut Box<dyn FileHandler>,
    pos: &PtPosition,
    id: u8,
) -> io::Result<Option<Arc<FileNode>>> {
    let root_info;
    match id {
        0x01 | 0x04 | 0x06 | 0x0b | 0x0c | 0x0e => {
            let mut fatfs = FatFs::new();
            fatfs.init(disk, pos)?;
            root_info = FatFs::get_root_info(Arc::new(fatfs));
        }
        _ => {
            return Ok(None);
        }
    };

    let fs_node = FileNode::new(name, FileType::FileSystem, root_info);
    let fs_node = Arc::new(fs_node);

    Ok(Some(fs_node))
}
