use std::{io, sync::Arc};

use fs_ops::mbr_fs_init;
use host_ops::FileHandler;
use mbr::read_mbr_partitions;

use crate::vfs::FileNode;

pub mod fs_ops;
pub mod host_ops;
pub mod utils;
pub mod mbr;

#[derive(Debug)]
pub enum PartitionError {
    ConvertError(String),
    IoError(String, io::Error),
    PartitionTableFull,
    NoValidPartition,
}

impl std::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for PartitionError {}

pub enum PartitionTableType {
    MBR,
    GPT,
}

#[derive(Clone)]
pub struct PtPosition {
    pub start: u64,
    pub end: u64,
}

pub fn read_partitions(disk: &mut Box<dyn FileHandler>) -> Result<Vec<Arc<FileNode>>, PartitionError> {
    let partitions = read_mbr_partitions(disk)?;

    let mut nodes = Vec::new();

    for (i, partition) in partitions.iter().enumerate() {
        match partition {
            Some((position, pt_type)) => {
                let node = mbr_fs_init("p".to_string() + &i.to_string(), disk, &position, *pt_type)
                    .map_err(|e| PartitionError::IoError("read_partitions".to_string(), e))?;
                if let Some(node) = node {
                    nodes.push(node);
                }
            },
            None => {},
        }
    }
    Ok(nodes)
}
