use std::fs::File;
use std::io::Read;

use super::PtPosition;
use super::{host_ops::FileHandler, PartitionError};
use super::utils::{lba_to_chs, to_sectors, SECTOR_SIZE};
use serde::{Serialize, Deserialize};
use bincode::{serialize, deserialize};

pub enum MbrPartitionType {
    Primary,
    Extended,
    Logical,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MbrPtItem {
    sign: u8,
    start_chs: [u8; 3],
    fs_type: u8,
    end_chs: [u8; 3],
    start_lba: u32,
    size: u32,
}

const MBR_BOOT_CODE: [u8; 75] = [
    0xFA, 0xB8, 0x00, 0x10, 0x8E, 0xD0, 0xBC, 0x00,
    0xB0, 0xB8, 0x00, 0x00, 0x8E, 0xD8, 0x8E, 0xC0,
    0xFB, 0xBE, 0x00, 0x7C, 0xBF, 0x00, 0x06, 0xB9,
    0x00, 0x02, 0xF3, 0xA4, 0xEA, 0x21, 0x06, 0x00,
    0x00, 0xBE, 0xBE, 0x07, 0x38, 0x04, 0x75, 0x0B,
    0x83, 0xC6, 0x10, 0x81, 0xFE, 0xFE, 0x07, 0x75,
    0xF3, 0xEB, 0x16, 0xB4, 0x02, 0xB0, 0x01, 0xBB,
    0x00, 0x7C, 0xB2, 0x80, 0x8A, 0x74, 0x01, 0x8B,
    0x4C, 0x02, 0xCD, 0x13, 0xEA, 0x00, 0x7C, 0x00,
    0x00, 0xEB, 0xFE];

const MBR_PARTITION_TABLE_OFFSET: usize = 446;
const MBR_PARTITION_ITEM_SIZE: usize = 16;
const MBR_PARTITION_TABLE_SIZE: usize = MBR_PARTITION_ITEM_SIZE * 4;

// 读取MBR分区表
pub(super) fn read_mbr_partitions(
    disk: &mut Box<dyn FileHandler>,
) -> Result<Vec<Option<(PtPosition, u8)>>, PartitionError> {
    let mut buf = [0u8; MBR_PARTITION_TABLE_SIZE];
    disk.seek(MBR_PARTITION_TABLE_OFFSET).map_err(|e| {
        PartitionError::IoError("CreateMbrPartition:定位MBR分区表时出错".to_string(), e)
    })?;
    disk.read(&mut buf).map_err(|e| {
        PartitionError::IoError("CreateMbrPartition:读取分区表时出错".to_string(), e)
    })?;

    let mut nodes = Vec::new();

    for part in buf.chunks(MBR_PARTITION_ITEM_SIZE) {
        let pt: MbrPtItem = deserialize(&part).unwrap();

        if pt.sign != 0x80 && pt.sign != 0x00 || pt.fs_type == 0x00 {
            nodes.push(None);
        }

        nodes.push(Some((PtPosition {
            start: pt.start_lba as u64,
            end: (pt.start_lba + pt.size) as u64,
        }, pt.fs_type)));
    }
    Ok(nodes)
}

// 创建MBR分区
pub fn create_mbr_partition(
    disk: &mut Box<dyn FileHandler>,
    ptype: MbrPartitionType,
    fs_type: u8,
    start: &String,
    end: &String,
    bootloader_path: Option<String>,
) -> Result<(), PartitionError> {
    let total_size = disk.total_size();
    let start = to_sectors(start, Some(63), Some(total_size)).ok_or(
        PartitionError::ConvertError("start:".to_string() + start),
    )? as u32;
    let end = to_sectors(end, Some(63), Some(total_size))
        .ok_or(PartitionError::ConvertError("end:".to_string() + end))? as u32;

    let mut sign = [0u8; 2];
    disk.seek(510).map_err(|e| {
        PartitionError::IoError("CreateMbrPartition:定位MBR签名时出错".to_string(), e)
    })?;
    disk.read(&mut sign).map_err(|e| {
        PartitionError::IoError("CreateMbrPartition:读取MBR签名时出错".to_string(), e)
    })?;


    let index;
    if sign != [0x55, 0xaa] {
        // 没有0x55，0xaa标志视作磁盘未初始化
        index = 0;
        disk.seek(510).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:写入签名MBR时定位出错".to_string(), e)
        })?;
        disk.write(&mut [0x55, 0xaa]).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:写入MBR签名时出错".to_string(), e)
        })?;
        disk.seek(0).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:定位MBR引导代码时出错".to_string(), e)
        })?;
        let mut boot_code = MBR_BOOT_CODE;
        disk.write(&mut boot_code).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:写入MBR引导代码时出错".to_string(), e)
        })?;
    } else {
        let partitions = read_mbr_partitions(disk).map_err(|e| {
            match e {
                PartitionError::IoError(string, err) => {
                    PartitionError::IoError(
                        "CreateMbrPartition->".to_string() + &string,
                        err)
                },
                _ => unreachable!()
            }
        })?;

        let mut uninitialized_partition = None;
        // 检测新分区的范围是否与已有分区重叠
        for (i, partition) in partitions.iter().enumerate() {
            match partition {
                Some(partition) => {
                    let start_lba = partition.0.start as u32;
                    let end_lba = partition.0.end as u32;
                    if start >= start_lba && start < end_lba || end > start_lba && end <= end_lba {
                        return Err(PartitionError::ConvertError(
                            "CreateMbrPartition:新分区范围与已有分区重叠".to_string(),
                        ));
                    }
                }
                None => {
                    uninitialized_partition = Some(i);
                }
            }
        }

        match uninitialized_partition {
            Some(i) => {index = i;},
            None => {
                return Err(PartitionError::PartitionTableFull);
            }
        };
    }

    let chs = disk.chs_info();
    let pt = MbrPtItem {
        sign: match ptype {
            MbrPartitionType::Primary => 0x80,
            MbrPartitionType::Extended => 0x0f,
            MbrPartitionType::Logical => 0x00,
        },
        start_chs: lba_to_chs(start, chs.1, chs.2),
        fs_type: fs_type,
        end_chs: lba_to_chs(end, chs.1, chs.2),
        start_lba: start,
        size: end - start,
    };

    disk.seek(MBR_PARTITION_TABLE_OFFSET + index * 16)
        .map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:定位新分区表项时出错".to_string(), e)
        })?;
    disk.write(&mut serialize(&pt).unwrap()).map_err(|e| {
        PartitionError::IoError("CreateMbrPartition:写入新分区表项时出错".to_string(), e)
    })?;

    if let Some(bootloader_path) = bootloader_path {
        let mut buf = [0u8; MBR_PARTITION_TABLE_OFFSET];
        let mut file = File::open(bootloader_path).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:打开引导程序时出错".to_string(), e)
        })?;
        file.read(&mut buf).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:读取引导程序时出错".to_string(), e)
        })?;
        
        disk.seek(start as usize * SECTOR_SIZE).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:定位引导程序时出错".to_string(), e)
        })?;
        disk.write(&mut buf).map_err(|e| {
            PartitionError::IoError("CreateMbrPartition:写入引导程序时出错".to_string(), e)
        })?;
    }

    Ok(())
}
