use std::io;

use crate::fs_ops::{self, FileSystem, Request};

use super::host_ops::FileHandler;
use bincode::deserialize;
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

const MBR_PARTITION_TABLE_OFFSET: usize = 446;
const MBR_PARTITION_TABLE_SIZE: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileType {
    File,
    Dir,
    Link,
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
    name: String,
    children: Vec<FileNode>,
    _ftype: FileType,
    pub fs: Box<dyn FileSystem>,
}

pub struct VFS {
    _root: FileNode,
}

impl FileNode {
    pub fn new_root(file: &mut Box<dyn FileHandler>) -> io::Result<Self> {
        let mut buf = [0u8; MBR_PARTITION_TABLE_SIZE];
        file.seek(MBR_PARTITION_TABLE_OFFSET);
        file.read(&mut buf)?;

        let mut root = FileNode::new("root".to_string(), FileType::Dir, Box::new(VfsOps));

        let mut i = 0;
        for part in buf.chunks(16) {
            let pt: MbrPtItem = deserialize(&part).unwrap();

            if pt.sign != 0x80 && pt.sign != 0x00 {
                continue;
            }
            let fs = match fs_ops::new(
                file,
                &PtPosition {
                    start: pt.start_lba as u64,
                    end: (pt.start_lba + pt.size) as u64,
                },
                pt.fs_type,
            )? {
                Some(fs) => fs,
                None => continue,
            };

            let name = "p".to_string() + i.to_string().as_str();
            let node = FileNode::new(name, FileType::Dir, fs);
            root.add_child(node);
            i += 1;
        }

        if root.children.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find a valid partition on the disk.",
            ))
        } else {
            Ok(root)
        }
    }

    pub fn new(name: String, ftype: FileType, fs: Box<dyn FileSystem>) -> Self {
        FileNode {
            name,
            children: Vec::new(),
            _ftype: ftype,
            fs,
        }
    }

    pub fn get_node(&mut self, dir: String) -> io::Result<&mut Self> {
        for i in self.children.iter_mut() {
            if i.name == dir {
                return Ok(i);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Node {dir} not found!"),
        ))
    }

    pub fn add_child(&mut self, child: Self) {
        self.children.push(child);
    }

    pub fn get_children(&mut self) -> &mut Vec<FileNode> {
        &mut self.children
    }
}

struct VfsOps;
impl FileSystem for VfsOps {
    fn init(&mut self, _disk: &mut Box<dyn FileHandler>, _pos: &PtPosition) -> io::Result<()> {
        Ok(())
    }
    fn open(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _path: String,
    ) -> io::Result<Request> {
        Ok(Request { offset: 0, idx: 0 })
    }
    fn read(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _req: &mut Request,
        _buf: &mut [u8],
        _size: usize,
    ) -> io::Result<usize> {
        Ok(0)
    }
    fn write(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _req: &mut Request,
        _buf: &mut [u8],
        _size: usize,
    ) -> io::Result<usize> {
        Ok(0)
    }
    fn create_file(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _name: &String,
        _ftype: FileType,
        _attr: u16,
        _create_date: &NaiveDate,
        _create_time: &NaiveTime,
        _write_date: &NaiveDate,
        _write_time: &NaiveTime,
        _last_acc_date: &NaiveDate,
        _file_size: u32,
    ) -> io::Result<Request> {
        Ok(Request { idx: 0, offset: 0 })
    }
    fn delete_file(
        &mut self,
        _disk: &mut Box<dyn FileHandler>,
        _req: &mut Request,
    ) -> io::Result<()> {
        Ok(())
    }
}

pub fn get_fs(
    root: &mut FileNode,
    target: String,
) -> io::Result<(String, &mut Box<dyn FileSystem>)> {
    let mut path: Vec<&str> = target.split("/").collect();
    while path[0] == "" {
        path.remove(0);
    }
    let node = root.get_node(path[0].to_string())?;
    path.remove(0);
    Ok((path.join("/"), &mut node.fs))
}
