use std::{
    fmt::Debug,
    io,
    path::{Component, Components, Path},
    sync::Arc,
};

use crate::{
    disk::{read_partitions, PartitionError},
    fs_ops::FileSystem,
};

use super::{
    disk::PtPosition,
    fs_ops::{fs_format, FileOps},
    host_ops::FileHandler,
};
use chrono::{NaiveDate, NaiveTime};
use lazy_static::lazy_static;
use std::sync::Mutex;

pub enum FileType {
    File,
    Directory,
    Link(Arc<FileNode>),
    FileSystem(PtPosition),
}

impl std::fmt::Debug for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "File"),
            Self::Directory => write!(f, "Dir"),
            Self::Link(_) => write!(f, "Link"),
            Self::FileSystem(_) => write!(f, "FileSystem"),
        }
    }
}

impl PartialEq for FileType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::File, Self::File) => true,
            (Self::Directory, Self::Directory) => true,
            (Self::Link(_), Self::Link(_)) => true,
            (Self::FileSystem(_), Self::FileSystem(_)) => true,
            _ => false,
        }
    }
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
    pub handler: Box<dyn FileHandler>,
}

impl VFS {
    pub fn instance() -> std::sync::MutexGuard<'static, Option<VFS>> {
        VFS_INSTANCE.lock().unwrap()
    }

    pub fn initialize(handler: Box<dyn FileHandler>) -> io::Result<()> {
        let root = FileNode::new(
            "root".to_string(),
            FileType::Directory,
            Mutex::new(Box::new(VfsFileOps)),
        );
        let root = Arc::new(root);
        let vfs = VFS { root, handler };

        let mut instance = VFS::instance();
        *instance = Some(vfs);
        Ok(())
    }

    pub fn load_image(&mut self) -> Result<(), PartitionError> {
        let fs_nodes = read_partitions(&mut self.handler).map_err(|e| match e {
            PartitionError::IoError(string, err) => {
                PartitionError::IoError("vfs::load_image->".to_string() + &string, err)
            }
            _ => unreachable!(),
        })?;

        for node in fs_nodes {
            self.root.add_child(node);
        }

        if self.root.children.lock().unwrap().is_empty() {
            return Err(PartitionError::NoValidPartition);
        }
        Ok(())
    }

    pub fn open(&mut self, path: &Path) -> io::Result<Arc<FileNode>> {
        let mut components = path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {
                    next = n;
                }
                None => {
                    break;
                }
            }
            match next {
                Component::Normal(name) => {
                    let found_node = {
                        let children = node.children.lock().unwrap();
                        children
                            .iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            })
                            .cloned()
                    };

                    match found_node {
                        Some(n) => {
                            node = n;
                        }
                        None => {
                            return Err(io::Error::new(
                                io::ErrorKind::AddrNotAvailable,
                                "找不到目录或文件".to_string() + name.to_str().unwrap(),
                            ));
                        }
                    }
                }
                _ => {}
            }

            match &node.ftype {
                FileType::FileSystem(_) => {
                    return node.handler.lock().unwrap().open(
                        &mut self.handler,
                        node.clone(),
                        components,
                    );
                }
                _ => {}
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "VFS::open未正常返回"))
    }

    pub fn create_file(
        &mut self,
        path: &Path,
        is_directory: bool,
        permission: u16,
        create_date: &NaiveDate,
        create_time: &NaiveTime,
        write_date: &NaiveDate,
        write_time: &NaiveTime,
        last_acc_date: &NaiveDate,
        file_size: u32,
    ) -> io::Result<Arc<FileNode>> {
        let mut components = path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {
                    next = n;
                }
                None => {
                    break;
                }
            }
            match next {
                Component::Normal(name) => {
                    let found_node = {
                        let children = node.children.lock().unwrap();
                        children
                            .iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            })
                            .cloned()
                    };

                    match found_node {
                        Some(n) => {
                            node = n;
                        }
                        None => {
                            return Err(io::Error::new(
                                io::ErrorKind::AddrNotAvailable,
                                "找不到目录或文件".to_string() + name.to_str().unwrap(),
                            ));
                        }
                    }
                }
                _ => {}
            }

            match &node.ftype {
                FileType::FileSystem(_) => {
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
                        file_size,
                    );
                }
                _ => {}
            }
        }
        Err(io::Error::new(
            io::ErrorKind::Other,
            "VFS::create_file未正常返回",
        ))
    }

    pub fn delete_file(&mut self, path: &Path) -> io::Result<()> {
        let mut components = path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {
                    next = n;
                }
                None => {
                    break;
                }
            }
            match next {
                Component::Normal(name) => {
                    let found_node = {
                        let children = node.children.lock().unwrap();
                        children
                            .iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            })
                            .cloned()
                    };

                    match found_node {
                        Some(n) => {
                            node = n;
                        }
                        None => {
                            return Err(io::Error::new(
                                io::ErrorKind::AddrNotAvailable,
                                "找不到目录或文件".to_string() + name.to_str().unwrap(),
                            ))?;
                        }
                    }
                }
                _ => {}
            }

            match &node.ftype {
                FileType::FileSystem(_) => {
                    return node.handler.lock().unwrap().delete_file(
                        &mut self.handler,
                        node.clone(),
                        components,
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn format_partition(
        &mut self,
        partition_path: &Path,
        fs_type: &str,
        options: Vec<String>,
    ) -> io::Result<()> {
        let mut components = partition_path.components();
        let mut node = self.root.clone();
        loop {
            let next;
            match components.next() {
                Some(n) => {
                    next = n;
                }
                None => {
                    break;
                }
            }
            match next {
                Component::Normal(name) => {
                    let found_node = {
                        let children = node.children.lock().unwrap();
                        children
                            .iter()
                            .find(|n| {
                                if let Some(name_str) = name.to_str() {
                                    n.name == name_str
                                } else {
                                    false
                                }
                            })
                            .cloned()
                    };

                    match found_node {
                        Some(n) => {
                            node = n;
                        }
                        None => {
                            return Err(io::Error::new(
                                io::ErrorKind::AddrNotAvailable,
                                "找不到目录或文件".to_string() + name.to_str().unwrap(),
                            ));
                        }
                    }
                }
                _ => {}
            }

            match &node.ftype {
                FileType::FileSystem(pos) => {
                    return fs_format(&mut self.handler, pos, fs_type, options);
                }
                _ => {}
            }
        }
        Err(io::Error::new(
            io::ErrorKind::Other,
            "VFS::format_partition未正常返回",
        ))
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
    fn init(&mut self, _disk: &mut Box<dyn FileHandler>, _pos: &PtPosition) -> io::Result<bool> {
        Err(io::Error::new(io::ErrorKind::Other, "VfsOps::init未实现"))
    }
    fn format_partition(
        _disk: &mut Box<dyn FileHandler>,
        _pos: &PtPosition,
        _fs_type: &str,
        _options: Vec<String>,
    ) -> io::Result<()> {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "VfsOps::format_partition未实现",
        ));
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VfsFileOps;
impl FileOps for VfsFileOps {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn open(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _fs_node: Arc<FileNode>,
        _path: Components,
    ) -> io::Result<Arc<FileNode>> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "VfsFileOps::open未实现",
        ))
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
        Err(io::Error::new(
            io::ErrorKind::Other,
            "VfsFileOps::create_file未实现",
        ))
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
