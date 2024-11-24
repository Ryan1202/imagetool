use std::any::Any;
use std::fmt::Debug;
use std::io;
use std::path::Components;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::{NaiveDate, NaiveTime};

use crate::disk::PtPosition;
use crate::host_ops::FileHandler;
use crate::vfs::FileType;

use self::fat::FatFs;

use crate::vfs::FileNode;
use crate::vfs::VfsFileOps;

pub mod fat;

pub struct Request {
    pub idx: usize,
    pub offset: usize,
}

pub trait FileSystem: Send + Sync {
    fn init(&mut self, disk: &mut Box<dyn FileHandler>, pos: &PtPosition) -> io::Result<bool>;

    fn format_partition(
        disk: &mut Box<dyn FileHandler>,
        pos: &PtPosition,
        fs_type: &str,
        options: Vec<String>,
    ) -> io::Result<()>;
}

pub trait FileOps: Send + Sync + Debug + Any {
    fn open(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        fs_root: Arc<FileNode>,
        path: Components,
    ) -> io::Result<Arc<FileNode>>;

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

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub fn fs_select_mbr_id(fs_type: &str) -> Option<u8> {
    match fs_type {
        "fat32" => FatFs::select_mbr_id(fs_type),
        _ => panic!("Unsupported file system"),
    }
}

pub fn mbr_fs_init(
    name: String,
    disk: &mut Box<dyn FileHandler>,
    pos: &PtPosition,
    id: u8,
) -> io::Result<Option<Arc<FileNode>>> {
    let root_info;
    let result;
    match id {
        0x01 | 0x04 | 0x06 | 0x0b | 0x0c | 0x0e => {
            let mut fatfs = FatFs::new();
            result = fatfs.init(disk, pos)?;
            root_info = FatFs::get_root_info(Arc::new(fatfs));
        }
        _ => {
            return Ok(None);
        }
    };

    if result {
        let fs_node = FileNode::new(name, FileType::FileSystem(pos.clone()), root_info);
        let fs_node = Arc::new(fs_node);
        Ok(Some(fs_node))
    } else {
        let fs_node = FileNode::new(
            name,
            FileType::FileSystem(pos.clone()),
            Mutex::new(Box::new(VfsFileOps)),
        );
        let fs_node = Arc::new(fs_node);
        Ok(Some(fs_node))
    }
}

pub fn fs_format(
    disk: &mut Box<dyn FileHandler>,
    pos: &PtPosition,
    fs_type: &str,
    options: Vec<String>,
) -> io::Result<()> {
    match fs_type {
        "fat32" => {
            FatFs::format_partition(disk, pos, fs_type, options)?;
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsupported file system",
            ));
        }
    }
    Ok(())
}
