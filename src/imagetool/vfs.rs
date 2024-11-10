use std::{fmt::Debug, io, path::{Component, Components, Path}, sync::Arc};

use crate::fs_ops::{self, FileSystem};

use super::{fs_ops::FileOps, host_ops::FileHandler};
use bincode::deserialize;
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use lazy_static::lazy_static;
use std::sync::Mutex;

const MBR_PARTITION_TABLE_OFFSET: usize = 446;
const MBR_PARTITION_TABLE_SIZE: usize = 64;

pub enum FileType {
    File,
    Directory,
    Link(Arc<FileNode>),
    FileSystem,
}

impl std::fmt::Debug for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "File"),
            Self::Directory => write!(f, "Dir"),
            Self::Link(_) => write!(f, "Link"),
            Self::FileSystem => write!(f, "FileSystem"),
        }
    }
}

impl PartialEq for FileType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::File, Self::File) => true,
            (Self::Directory, Self::Directory) => true,
            (Self::Link(_), Self::Link(_)) => true,
            (Self::FileSystem, Self::FileSystem) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct MbrPtItem {
    sign: u8,
    start_chs: [u8; 3],
    fs_type: u8,
    end_chs: [u8; 3],
    start_lba: u32,
    size: u32,
}

pub struct PtPosition {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug)]
pub struct FileNode {
    pub(crate) name: String,
    pub(crate) children: Mutex<Vec<Arc<FileNode>>>,
    pub(crate) ftype: FileType,
    pub handler: Mutex<Box<dyn FileOps>>,
}

lazy_static! {
    static ref VFS_INSTANCE: Mutex<Option<VFS>> = Mutex::new(None);
}

pub struct VFS {
    root: Arc<FileNode>,
    pub handler: Box<dyn FileHandler>
}

impl VFS {
    pub fn instance() -> std::sync::MutexGuard<'static, Option<VFS>> {
        VFS_INSTANCE.lock().unwrap()
    }

    pub fn initialize(handler: Box<dyn FileHandler>) -> io::Result<()> {
        let root = FileNode::new("root".to_string(), FileType::Directory, Mutex::new(Box::new(VfsFileOps)));
        let root = Arc::new(root);
        let mut vfs = VFS { root, handler };
        
        vfs.load_image()?;

        let mut instance = VFS::instance();
        *instance = Some(vfs);
        Ok(())
    }
    
    fn load_image(&mut self) -> io::Result<()> {
        let mut buf = [0u8; MBR_PARTITION_TABLE_SIZE];
        self.handler.seek(MBR_PARTITION_TABLE_OFFSET)?;
        self.handler.read(&mut buf)?;

        let mut i = 0;
        for part in buf.chunks(16) {
            let pt: MbrPtItem = deserialize(&part).unwrap();

            if pt.sign != 0x80 && pt.sign != 0x00 || pt.fs_type == 0x00 {
                continue;
            }
            
            let name = "p".to_string() + i.to_string().as_str();
            let fs_node = match fs_ops::fs_init(
                name,
                &mut self.handler,
                &PtPosition {
                    start: pt.start_lba as u64,
                    end: (pt.start_lba + pt.size) as u64,
                },
                pt.fs_type,
            )? {
                Some(fs) => fs,
                None => continue,
            };

            self.root.add_child(fs_node);
            i += 1;
        }

        if self.root.children.lock().unwrap().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find a valid partition on the disk.",
            ))
        }
        Ok(())
    }

    pub fn open(&mut self, path: &Path) -> io::Result<Arc<FileNode>> {
        let mut components = path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {next = n;},
                None => {break;},
            }
            match next {
                Component::Normal(name) => {
                    let found_node ={
                        let children = node.children.lock().unwrap();
                        children.iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            }).cloned()
                    };

                    match found_node {
                        Some(n) => {node = n;},
                        None => {
                            return Err(io::Error::new(io::ErrorKind::AddrNotAvailable, "找不到目录或文件".to_string()+name.to_str().unwrap()));
                        },
                    }
                },
                _ => {},
            }

            match &node.ftype {
                FileType::FileSystem => {
                    return node.handler.lock().unwrap().open(&mut self.handler, node.clone(), components);
                },
                _ => {}
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "VFS::open未正常返回"))
    }

    pub fn create_file(&mut self, path: &Path, is_directory: bool, permission: u16, create_date: &NaiveDate, create_time: &NaiveTime, write_date: &NaiveDate, write_time: &NaiveTime, last_acc_date: &NaiveDate, file_size: u32) -> io::Result<Arc<FileNode>> {
        let mut components = path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {next = n;},
                None => {break;},
            }
            match next {
                Component::Normal(name) => {
                    let found_node = {
                        let children = node.children.lock().unwrap();
                        children.iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            }).cloned()
                    };

                    match found_node {
                        Some(n) => {node = n;},
                        None => {
                            return Err(io::Error::new(
                                io::ErrorKind::AddrNotAvailable,
                                "找不到目录或文件".to_string()+name.to_str().unwrap()));
                        },
                    }
                },
                _ => {},
            }

            match &node.ftype {
                FileType::FileSystem => {
                    return node.handler.lock().unwrap().create_file(
                        &mut self.handler,
                        node.clone(),
                        components,
                        is_directory,
                        permission,
                        create_date,
                        create_time,
                        write_date,
                        write_time,
                        last_acc_date,
                        file_size);
                },
                _ => {

                }
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "VFS::create_file未正常返回"))
    }

    pub fn delete_file(&mut self, path: &Path) -> io::Result<()> {
        let mut components = path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {next = n;},
                None => {break;},
            }
            match next {
                Component::Normal(name) => {
                    let found_node = {
                        let children = node.children.lock().unwrap();
                        children.iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            }).cloned()
                    };

                    match found_node {
                        Some(n) => {node = n;},
                        None => {
                            return Err(io::Error::new(io::ErrorKind::AddrNotAvailable, "找不到目录或文件".to_string()+name.to_str().unwrap()))?;
                        },
                    }
                },
                _ => {},
            }

            match &node.ftype {
                FileType::FileSystem => {
                    return node.handler.lock().unwrap().delete_file(&mut self.handler, node.clone(), components);
                },
                _ => {}
            }
        }
        Ok(())
    }
}

impl FileNode {
    pub fn new(name: String, ftype: FileType, handler: Mutex<Box<dyn FileOps>>) -> Self {
        FileNode {
            name,
            children: Mutex::new(Vec::new()),
            ftype,
            handler: handler,
        }
    }

    pub fn add_child(&self, child: Arc<Self>) {
        self.children.lock().unwrap().push(child);
    }
}

#[derive(Debug, Clone)]
struct VfsOps;
impl FileSystem for VfsOps {
    fn init(&mut self, _disk: &mut Box<dyn FileHandler>, _pos: &PtPosition) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct VfsFileOps;
impl FileOps for VfsFileOps {
    fn open(&mut self, _disk: &mut Box<dyn FileHandler>, _fs_node: Arc<FileNode>, _path: Components) -> io::Result<Arc<FileNode>> {
        Err(io::Error::new(io::ErrorKind::Other, "VfsFileOps::open未实现"))
    }
    fn create_file(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _fs_root: Arc<FileNode>,
        _path: Components,
        _is_directory: bool,
        _permission: u16,
        _create_date: &NaiveDate,
        _create_time: &NaiveTime,
        _write_date: &NaiveDate,
        _write_time: &NaiveTime,
        _last_acc_date: &NaiveDate,
        _file_size: u32,
    ) -> io::Result<Arc<FileNode>> {
        Err(io::Error::new(io::ErrorKind::Other, "VfsFileOps::create_file未实现"))
    }
    fn delete_file(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _fs_root: Arc<FileNode>,
        _path: Components,
    ) -> io::Result<()> {
        Ok(())
    }
    fn read(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _size: usize,
        _buf: &mut [u8],
    ) -> io::Result<usize> {
        Ok(0)
    }
    fn write(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _size: usize,
        _buf: &mut [u8],
    ) -> io::Result<usize> {
        Ok(0)
    }
}
