use bincode::deserialize;
use byteorder::{ByteOrder, LittleEndian};
use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::cmp::min;
use std::path::{Component, Components};
use std::sync::{Arc, Mutex};
use std::{io, usize, vec};

use self::dir::{
    DIR_ATTR, DIR_CRT_DATE, DIR_CRT_TIME, DIR_CRT_TIME_TENTH, DIR_FILE_SIZE, DIR_FST_CLUS_HI,
    DIR_FST_CLUS_LO, DIR_LST_ACC_DATE, DIR_NAME, DIR_NTRES, DIR_WRT_DATE, DIR_WRT_TIME,
};

use super::{FileOps, FileSystem};
use crate::host_ops::FileHandler;
use crate::utils::{ceil_div, SECTOR_SIZE};
use crate::vfs::{FileNode, FileType, PtPosition};

const ROOT_CLUSTER: u32 = 2;

mod bpb {
    // FAT的引导扇区和BOOT INFO扇区中的部分偏移地址
    pub(super) const _BS_JMP_BOOT: usize = 0;

    pub(super) const _BS_OEM_NAME: usize = 3;

    pub(super) const _BYTS_PER_SEC: usize = 11;

    pub(super) const _SEC_PER_CLUS: usize = 13;

    pub(super) const _RSVD_SEC_CNT: usize = 14;

    pub(super) const _NUM_FATS: usize = 16;

    pub(super) const _ROOT_ENT_CNT: usize = 17;

    pub(super) const _TOT_SEC16: usize = 19;

    pub(super) const _MEDIA: usize = 21;

    pub(super) const _FAT_SZ16: usize = 22;

    pub(super) const _SEC_PER_TRK: usize = 24;

    pub(super) const _NUM_HEADS: usize = 26;

    pub(super) const _HIDD_SEC: usize = 28;

    pub(super) const _TOT_SEC32: usize = 32;

    pub(super) const _FAT_SZ32: usize = 36;
}
mod dir {
    // FAT短目录项和长目录项的数据结构
    pub(super) const LDIR_ORD: usize = 0;

    pub(super) const LDIR_NAME1: usize = 1;

    pub(super) const LDIR_ATTR: usize = 11;

    pub(super) const LDIR_TYPE: usize = 12;

    pub(super) const LDIR_CHKSUM: usize = 13;

    pub(super) const LDIR_NAME2: usize = 14;

    pub(super) const _LDIR_FST_CLUS_LO: usize = 26;

    pub(super) const LDIR_NAME3: usize = 28;

    pub(super) const DIR_NAME: usize = 0;

    pub(super) const DIR_ATTR: usize = 11;

    pub(super) const DIR_NTRES: usize = 12;

    pub(super) const DIR_CRT_TIME_TENTH: usize = 13;

    pub(super) const DIR_CRT_TIME: usize = 14;

    pub(super) const DIR_CRT_DATE: usize = 16;

    pub(super) const DIR_LST_ACC_DATE: usize = 18;

    pub(super) const DIR_FST_CLUS_HI: usize = 20;

    pub(super) const DIR_WRT_TIME: usize = 22;

    pub(super) const DIR_WRT_DATE: usize = 24;

    pub(super) const DIR_FST_CLUS_LO: usize = 26;

    pub(super) const DIR_FILE_SIZE: usize = 28;
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FatFsType {
    FAT12,
    FAT16,
    FAT32,
}

enum FileNameType {
    LongName,
    ShortName,
}

#[derive(Clone, Debug)]
pub struct FatFs {
    /// FAT表大小
    fat_size: u32,
    /// 总扇区数
    tot_sec: u32,
    /// 数据部分扇区数
    data_sec: u32,
    /// FAT表起始扇区
    fat_start: u32,
    /// 数据部分起始扇区
    data_start: u32,
    /// 有效簇号的最大值
    max_clus: u32,
    /// 每扇区字节数
    bytes_per_clus: usize,
    /// 每簇扇区数
    sec_per_clus: usize,
    /// 每扇区字节数
    bytes_per_sec: usize,

    fs_type: FatFsType,
    bpb: BPB,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BPB {
    boot_jmp: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sec: u16,
    sec_per_clus: u8,
    rsvd_sec_cnt: u16,
    num_fats: u8,
    root_ent_cnt: u16,
    tot_sec16: u16,
    media: u8,
    fat_sz16: u16,
    sec_per_trk: u16,
    num_heads: u16,
    hidd_sec: u32,
    tot_sec32: u32,
    fat_sz32: u32,
    ext_flags: u16,
    fs_ver: u16,
    root_clus: u32,
    fs_info: u16,
    bk_boot_sec: u16,
    reserved: [u8; 12],
    drv_num: u8,
    reserved1: u8,
    boot_sig: u8,
    vol_id: u32,
    vol_lab: [u8; 11],
    fil_sys_type: [u8; 8],
    #[serde(with = "BigArray")]
    boot_code: [u8; 420],
    signature: u16,
}

impl BPB {
    pub fn new_empty() -> Self {
        Self {
            boot_jmp: [0u8; 3],
            oem_name: [0u8; 8],
            bytes_per_sec: 0,
            sec_per_clus: 0,
            rsvd_sec_cnt: 0,
            num_fats: 0,
            root_ent_cnt: 0,
            tot_sec16: 0,
            media: 0,
            fat_sz16: 0,
            sec_per_trk: 0,
            num_heads: 0,
            hidd_sec: 0,
            tot_sec32: 0,
            fat_sz32: 0,
            ext_flags: 0,
            fs_ver: 0,
            root_clus: 0,
            fs_info: 0,
            bk_boot_sec: 0,
            reserved: [0u8; 12],
            drv_num: 0,
            reserved1: 0,
            boot_sig: 0,
            vol_id: 0,
            vol_lab: [0u8; 11],
            fil_sys_type: [0u8; 8],
            boot_code: [0u8; 420],
            signature: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct ShortDir {
    name: ShortName,
    attr: u8,
    ntres: u8,
    create_time_tenth: u8,
    create_time: u16,
    create_date: u16,
    last_acc_date: u16,
    first_clus_hi: u16,
    write_time: u16,
    write_date: u16,
    first_clus_lo: u16,
    file_size: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct LongDir {
    ord: u8,
    name1: [u16; 5],
    attr: u8,
    ftype: u8,
    chksum: u8,
    name2: [u16; 6],
    first_clus_lo: u16,
    name3: [u16; 2],
}

#[derive(Debug, Clone)]
struct ExtendInfo {
    fs: Arc<FatFs>,
    // 目录项所在的簇号
    directory_cluster: u32,
    // 目录项在簇中的序号
    directory_num: u8,

    offset: u32,
    cluster_list: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ShortName {
    base_name: [u8; 8],
    ext_name: [u8; 3],
}

impl ShortName {
    fn new(name: &String, fs: &FatFs) -> io::Result<Self> {
        if !fs.check_short_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Illegal short name",
            ));
        }
        let name = name.to_owned().to_uppercase();
        let parts: Vec<&str> = name.rsplitn(2, '.').collect();
        let mut base_arr = [b' '; 8];
        let mut ext_arr = [b' '; 3];

        match parts.as_slice() {
            [ext, name] => {
                for (i, &byte) in name.as_bytes().iter().enumerate().take(8) {
                    base_arr[i] = byte;
                }
                for (i, &byte) in ext.as_bytes().iter().enumerate().take(3) {
                    ext_arr[i] = byte;
                }
            }
            [name] => {
                // 没有扩展名
                for (i, &byte) in name.as_bytes().iter().enumerate().take(8) {
                    base_arr[i] = byte;
                }
            }
            _ => {}
        }

        Ok(Self {
            base_name: base_arr,
            ext_name: ext_arr,
        })
    }

    fn from_u8_slice(name: &[u8; 8], ext: &[u8; 3]) -> io::Result<Self> {
        Ok(Self {
            base_name: name.to_owned(),
            ext_name: ext.to_owned(),
        })
    }
}

impl ShortDir {
    fn new(
        name: ShortName,
        full_name: &String,
        attribute: u8,
        ctime_tenth: u8,
        ctime: u16,
        cdate: u16,
        lacc_date: u16,
        first_clus: u32,
        wtime: u16,
        wdate: u16,
        file_size: u32,
    ) -> io::Result<Self> {
        let caps = check_fname_caps(full_name);
        let nt_res = if caps & 0x03u8 == 0x01 { BASE_L } else { 0 }
            | if caps & 0x0cu8 == 0x04 { EXT_L } else { 0 };
        Ok(Self {
            name,
            attr: attribute,
            ntres: nt_res,
            create_time_tenth: ctime_tenth,
            create_time: ctime,
            create_date: cdate,
            last_acc_date: lacc_date,
            first_clus_hi: (first_clus >> 16) as u16,
            write_time: wtime,
            write_date: wdate,
            first_clus_lo: (first_clus & 0xffff) as u16,
            file_size,
        })
    }

    fn to_bytes(&self, first_clus: u32) -> [u8; 32] {
        let mut buf = [0u8; 32];
        for (i, &ch) in self.name.base_name.iter().enumerate() {
            buf[DIR_NAME + i] = ch;
        }
        for (i, &ch) in self.name.ext_name.iter().enumerate() {
            buf[DIR_NAME + 8 + i] = ch;
        }
        buf[DIR_ATTR] = self.attr;
        buf[DIR_NTRES] = self.ntres;
        buf[DIR_CRT_TIME_TENTH] = self.create_time_tenth;
        buf[DIR_CRT_TIME + 0] = self.create_time as u8;
        buf[DIR_CRT_TIME + 1] = (self.create_time >> 8) as u8;
        buf[DIR_CRT_DATE + 0] = self.create_date as u8;
        buf[DIR_CRT_DATE + 1] = (self.create_date >> 8) as u8;
        buf[DIR_LST_ACC_DATE + 0] = self.last_acc_date as u8;
        buf[DIR_LST_ACC_DATE + 1] = (self.last_acc_date >> 8) as u8;
        buf[DIR_FST_CLUS_LO + 0] = first_clus as u8;
        buf[DIR_FST_CLUS_LO + 1] = (first_clus >> 8) as u8;
        buf[DIR_FST_CLUS_HI + 0] = (first_clus >> 16) as u8;
        buf[DIR_FST_CLUS_HI + 1] = (first_clus >> 24) as u8;
        buf[DIR_WRT_TIME + 0] = self.write_time as u8;
        buf[DIR_WRT_TIME + 1] = (self.write_time >> 8) as u8;
        buf[DIR_WRT_DATE + 0] = self.write_date as u8;
        buf[DIR_WRT_DATE + 1] = (self.write_date >> 8) as u8;
        buf[DIR_FILE_SIZE + 0] = self.file_size as u8;
        buf[DIR_FILE_SIZE + 1] = (self.file_size >> 8) as u8;
        buf[DIR_FILE_SIZE + 2] = (self.file_size >> 16) as u8;
        buf[DIR_FILE_SIZE + 3] = (self.file_size >> 24) as u8;

        buf
    }

    fn match_short_dir(buf: &[u8; 32], name: &String) -> bool {
        let mut j = 0;
        let name_bytes = name.as_bytes();
        let mut flag = true;
        let len = name.len();

        // 匹配文件名
        for &x in buf[0..8].into_iter() {
            if x == 0x20 {
                // 为空格
                if j < name_bytes.len() {
                    if name_bytes[j] != 0x20 {
                        if name_bytes[j] == '.' as u8 {
                            break;
                        } else {
                            flag = false;
                        }
                    }
                }
            } else if x.is_ascii_alphabetic() {
                //为字母
                if buf[12] & BASE_L != 0 {
                    if j < len && x.to_ascii_lowercase() != name_bytes[j] {
                        flag = false;
                    }
                } else {
                    if j >= len || x != name_bytes[j] {
                        flag = false;
                    }
                }
            } else if x.is_ascii_digit() {
                //为数字
                if j >= len || x != name_bytes[j] {
                    flag = false;
                }
            } else {
                if is_short_name_available_char(name_bytes[j] as char) {
                    if x != name_bytes[j] {
                        flag = false;
                    }
                }
            }
            if flag {
                j+=1;
                continue;
            } else {
                return false;
            }
        }
        // 匹配扩展名
        if buf[8..11] != [0x20, 0x20, 0x20] {
            j += 1;
            for &x in buf[8..11].into_iter() {
                if x.is_ascii_alphabetic() {
                    if buf[DIR_NTRES] & EXT_L != 0 {
                        if j >= len || x.to_ascii_lowercase() != name_bytes[j] {
                            flag = false;
                        }
                    } else {
                        if j >= len || x != name_bytes[j] {
                            flag = false;
                        }
                    }
                } else if x.is_ascii_digit() {
                    if j >= len || x != name_bytes[j] {
                        flag = false
                    }
                } else if x == 0x20 {
                    if j < name_bytes.len() && x != name_bytes[j] {
                        flag = false;
                    }
                } else {
                    if is_short_name_available_char(name_bytes[j] as char) {
                        if x != name_bytes[j] {
                            flag = false;
                        }
                    }
                }
                if flag {
                    j+=1;
                    continue;
                } else {
                    return false;
                }
            }
        } else {
            if j < name_bytes.len() {
                return false;
            }
        }

        true
    }
}

impl FileSystem for FatFs {
    fn init(&mut self, disk: &mut Box<dyn FileHandler>, pos: &PtPosition) -> io::Result<()> {
        let mut buf = [0u8; SECTOR_SIZE];
        disk.seek(pos.start as usize * SECTOR_SIZE)?;
        disk.read(&mut buf)?;

        let bpb: BPB = deserialize(&buf).unwrap();

        let fatsz: u32;
        if bpb.fat_sz16 != 0 {
            fatsz = bpb.fat_sz16.into();
        } else {
            fatsz = bpb.fat_sz32;
        }

        let total_sec: u32;
        if bpb.tot_sec16 != 0 {
            total_sec = bpb.tot_sec16.into();
        } else {
            total_sec = bpb.tot_sec32;
        }

        let root_dir_sectors: u32 = if bpb.root_ent_cnt == 0 {
            0
        } else {
            ((bpb.root_ent_cnt as u32 * 32) + (bpb.bytes_per_sec - 1) as u32)
                / bpb.bytes_per_sec as u32
        };
        let fat_start: u32 = pos.start as u32 + bpb.rsvd_sec_cnt as u32 + root_dir_sectors;
        let data_start: u32 = (bpb.num_fats as u32 * fatsz) + fat_start;
        let data_sec = total_sec - data_start;

        let count_of_clusters = data_sec / bpb.sec_per_clus as u32;
        let fs_type = if count_of_clusters < 4085 {
            FatFsType::FAT12
        } else if count_of_clusters < 65525 {
            FatFsType::FAT16
        } else {
            FatFsType::FAT32
        };

        self.fat_size = fatsz;
        self.tot_sec = total_sec;
        self.data_sec = data_sec;
        self.fat_start = fat_start;
        self.data_start = data_start;
        self.fs_type = fs_type;
        self.max_clus = count_of_clusters + 1;
        self.bytes_per_sec = bpb.bytes_per_sec as usize;
        self.sec_per_clus = bpb.sec_per_clus as usize;
        self.bytes_per_clus = self.bytes_per_sec * self.sec_per_clus;
        self.bpb = bpb;

        Ok(())
    }
}

impl FileOps for ExtendInfo {
    fn open(&mut self, disk: &mut Box<dyn FileHandler>, fs_root: Arc<FileNode>, path: Components) -> io::Result<Arc<FileNode>> {        
        let mut path = path;
        
        let (mut node, mut extend_info) = self.open_file_in_directory(disk, fs_root, path.next().unwrap())?;
        for component in path {
            (node, extend_info) = extend_info.open_file_in_directory(disk, node, component)?;
        }
        Ok(node)
    }

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
    ) -> io::Result<Arc<FileNode>> {        
        let mut node = fs_root;
        let mut extend_info = self.clone();

        let mut path = path;
        let mut component = path.next();
        while component.is_some() {
            let next = path.next();
            match component.unwrap() {
                Component::Normal(name) => {
                    let name = name.to_str()
                        .ok_or(
                        io::Error::new(
                            io::ErrorKind::Other,
                            "不支持的文件名：".to_string()
                                + name.to_str().unwrap()))?;
                    match extend_info.open_file_in_directory(disk, node.clone(), component.unwrap()) {
                        Ok((new_node, new_extend_info)) => {
                            node = new_node;
                            extend_info = new_extend_info;
                        },
                        Err(err) => {
                            if let io::ErrorKind::NotFound = err.kind() {
                                let (new_node, new_extend_info) =
                                    extend_info.create_entry_in_directory(
                                        disk,
                                        node.clone(),
                                        &name.to_string(),
                                        is_directory || next.is_some(),
                                        permission,
                                        create_date,
                                        create_time,
                                        write_date,
                                        write_time,
                                        last_acc_date,
                                        file_size)?;
                                node.add_child(new_node.clone());
                                (node, extend_info) = (new_node, new_extend_info);
                            } else {
                                return Err(err);
                            }
                        },
                    };
                },
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "无效文件或文件夹名!".to_string()
                            + component.unwrap().as_os_str().to_str().unwrap(),
                    ));
                }
            }
            component = next;
        }
        Ok(node)
    }

    fn delete_file(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        fs_root: Arc<FileNode>,
        path: Components,
    ) -> io::Result<()> {
        let mut path = path;
        
        let (mut node, mut extend_info) = self.open_file_in_directory(disk, fs_root, path.next().unwrap())?;
        for component in path {
            (node, extend_info) = extend_info.open_file_in_directory(disk, node, component)?;
        }

        let fs = extend_info.fs.clone();
        
        let mut buf = [0u8; 32];
        // 读取文件对应的表项
        fs.read_dir_entry(
            disk,
            extend_info.directory_cluster,
            extend_info.directory_num,
            &mut buf,
        )?;
        // 标记为已删除
        buf[0] = 0xe5;
        // 写入
        fs.write_dir_entry(
            disk,
            extend_info.directory_cluster,
            extend_info.directory_num,
            &mut buf,
        )?;
        Ok(())
    }

    fn read(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        size: usize,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        let fs = self.fs.clone();
        
        let range = fs.file_range(disk, self, self.offset as usize, size)?;
        let mut done = 0;
        for (start, end) in range {
            let length = end - start;
            let mut tmp_buf = Vec::with_capacity(length);
            tmp_buf.resize(length, 0);
            disk.seek(start)?;
            disk.read(&mut tmp_buf)?;
            buf[done..done + length].copy_from_slice(&tmp_buf);
            done += length;
        }
        self.offset += done as u32;
        Ok(done)
    }

    fn write(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        size: usize,
        buf: &mut [u8],
    )-> io::Result<usize> {
        let fs = self.fs.clone();
        
        let range = fs.file_range(disk, self, self.offset as usize, size)?;
        let mut done = 0;
        for (start, end) in range {
            let length = end - start;
            disk.seek(start)?;
            disk.write(&mut buf[done..(done+length)])?;
            done += length;
            self.offset += length as u32;
        }
        Ok(done)
    }
}

impl ExtendInfo {
    fn open_file_in_directory(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        node: Arc<FileNode>,
        component: Component,
    ) -> io::Result<(Arc<FileNode>, Self)> {
        let mut node = node;
        let mut extend_info = self.clone();

        match component {
            Component::Normal(name) => {
                let name = name.to_str()
                    .ok_or(
                    io::Error::new(
                        io::ErrorKind::Other,
                        "不支持的文件名：".to_string()
                            + name.to_str().unwrap()))?;

                let new_node = {
                    let children = node.children.lock().unwrap();
                    children.iter().find(|x| x.name == name).cloned()
                };
                match new_node {
                    Some(n) => {
                        node = n.clone();
                    },
                    None => {
                        (node, extend_info) = extend_info.search_directory(disk, name)?;
                    }
                }
            },
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "无效文件或文件夹名!".to_string()+component.as_os_str().to_str().unwrap(),
                ));
            }
        }
        
        Ok((node, extend_info))
    }
    

    fn search_directory(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        name: &str,
    ) -> io::Result<(Arc<FileNode>, ExtendInfo)> {
        let fs = self.fs.clone();
        let mut clus_i = 0;
        let clus = &self.cluster_list;

        let mut buf = [0u8; 0x20];
        let mut i: i32 = -1;
        let mut flag: bool = false;
        let mut result: Option<[u8; 32]> = None;

        let mut new_info = ExtendInfo {
            fs: fs.clone(),
            directory_cluster: clus[clus_i],
            directory_num: 0,
            offset: 0,
            cluster_list: vec![],
        };

        loop {
            i += 1;
            if i as usize >= clus.len() * (fs.bytes_per_clus / 0x20) {
                break;
            }
            fs.read_dir_entry(disk, clus[clus_i], i as u8, &mut buf)?;
            if buf[0] == 0xe5 || buf[0] == 0x00 || buf[0] == 0x05 {
                continue;
            }
            let mut ldir: LongDir = deserialize(&buf).unwrap();

            let mut fname = String::new();
            if let FatFsType::FAT32 = fs.fs_type {
                let chksum = ldir.chksum;
                while ldir.attr == ATTR_LONG_NAME && ldir.ord != 0xe5 {
                    prepend_utf16_to_string(&ldir.name3, &mut fname);
                    prepend_utf16_to_string(&ldir.name2, &mut fname);
                    prepend_utf16_to_string(&ldir.name1, &mut fname);

                    i += 1;
                    if i as u32 > (fs.bytes_per_clus as u32 / 0x20) {
                        clus_i += 1;
                        i = 0;
                    }
                    fs.read_dir_entry(disk, clus[clus_i], i as u8, &mut buf)?;
                    ldir = deserialize(&buf).unwrap();
                    if ldir.ord & 0x0f == 0x01 || ldir.chksum != chksum {
                        break;
                    }
                }
                if !fname.is_empty() && fname.to_uppercase() == name.to_uppercase() {
                    if ShortName::from_u8_slice(
                        buf[..8].try_into().unwrap(),
                        buf[8..11].try_into().unwrap(),
                    )?
                    .cal_chksum()?
                        != chksum
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Incorrect checksum!",
                        ));
                    }
                    flag = true;
                    result = Some(buf);
                    break;
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Unsupported FAT fs!",
                ));
            }

            if name.is_ascii() {
                if ShortDir::match_short_dir(&buf[0..32].try_into().unwrap(), &name.to_string()) {
                    flag = true;
                    result = Some(buf);
                    break;
                }
                flag = false;
            }
        }
        if flag == true {
            match result {
                Some(sdir) => {
                    let clus = ((LittleEndian::read_u16(&sdir[20..22]) as u32) << 16)
                        | (LittleEndian::read_u16(&sdir[26..28]) as u32);
                    
                    new_info.directory_cluster = self.cluster_list[clus_i];
                    new_info.directory_num = i as u8 % (fs.sec_per_clus as u8 * 16);
                    new_info.offset = i as u32;
                    new_info.cluster_list = fs.get_all_clus(disk, clus)?;
                    let new_node = FileNode::new(name.to_owned(), FileType::File, Mutex::new(Box::new(new_info.clone())));
                    return Ok((Arc::new(new_node), new_info));
                }
                None => {},
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "找不到文件或文件夹：".to_string()+name))
    }

    fn create_entry_in_directory(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        parent_node: Arc<FileNode>,
        name: &String,
        is_directory: bool,
        permission: u16,
        create_date: &NaiveDate,
        create_time: &NaiveTime,
        write_date: &NaiveDate,
        write_time: &NaiveTime,
        last_acc_date: &NaiveDate,
        file_size: u32,
    ) -> io::Result<(Arc<FileNode>, ExtendInfo)> {
        let fs = self.fs.clone();

        // 转换时间格式
        let cdate = to_fat32_date(create_date);
        let ctime = to_fat32_time(create_time);
        let wdate = to_fat32_date(write_date);
        let wtime = to_fat32_time(write_time);
        let lacc_date = to_fat32_date(last_acc_date);
        let ctime_tenth = to_fat32_time_tenth(create_time);

        // 转换属性格式
        let mut attribute = ATTR_ARCHIVE | if is_directory {
            ATTR_DIRECTORY
        } else {
            0
        };

        if permission & 0b011_011_011 == 0b011_011_011 {
            attribute |= ATTR_READ_ONLY
        }

        // 为新目录项分配簇
        let first_clus = fs.alloc_clus(disk, 0, true)?;

        let (mut entry, blocks) = self.create_entry_block(
            disk,
            &name,
            attribute,
            ctime_tenth,
            ctime,
            cdate,
            lacc_date,
            first_clus,
            wtime,
            wdate,
            file_size,
        )?;

        // 写入目录项
        let mut clus;
        let mut num;
        for mut i in blocks {
            (clus, num) = fs.new_dir_entry(disk, &self)?;
            fs.write_dir_entry(disk, clus, num, &mut i)?;
            entry.directory_cluster = clus;
            entry.directory_num = num;
        }
        
        entry.cluster_list.push(first_clus);
        
        let new_node = Arc::new(FileNode::new(name.to_owned(),
            if is_directory {FileType::Directory} else {FileType::File},
            Mutex::new(Box::new(entry.clone()))));
        if self.directory_cluster < 0xfffffff0 {
            parent_node.children.lock().unwrap().push(new_node.clone());
        }

        // 如果是文件夹则要创建'.'和'..'
        if is_directory {
            // 创建'.'
            let mut short_dir = ShortDir::new(
                ShortName { base_name: *b".       ", ext_name: *b"   " },
                &".".to_string(),
                ATTR_ARCHIVE | ATTR_DIRECTORY,
                ctime_tenth,
                ctime,
                cdate,
                lacc_date,
                first_clus,
                wtime,
                wdate,
                file_size)?;
            fs.write_dir_entry(disk, first_clus, 0, &mut short_dir.to_bytes(first_clus))?;
            // 创建'..'
            short_dir.name = ShortName { base_name: *b"..      ", ext_name: *b"   " };
            fs.write_dir_entry(disk, first_clus, 1, &mut short_dir.to_bytes(self.cluster_list[0]))?;
        }

        Ok((new_node, entry))
    }
    /// 创建文件目录项
    /// 备注：该函数仅创建目录项，并未设置directory_cluster和directory_num
    fn create_entry_block(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        name: &String,
        attribute: u8,
        ctime_tenth: u8,
        ctime: u16,
        cdate: u16,
        lacc_date: u16,
        first_clus: u32,
        wtime: u16,
        wdate: u16,
        file_size: u32,
    ) -> io::Result<(ExtendInfo, Vec<[u8; 32]>)> {
        let fs = self.fs.clone();
        let name_type = fs.check_name(name)?;
        let mut blocks: Vec<[u8; 32]> = Vec::new();
        let short_name = match name_type {
            FileNameType::LongName => self.long_name2short_name(disk, name)?,
            FileNameType::ShortName => ShortName::new(name, &fs)?,
        };

        if let FileNameType::LongName = name_type {
            let mut long_name = name.clone();
            long_name.push('\0');
            let mut buf = [0u8; 32];
            let mut ord = 1;
            let mut i = 0;
            let chksum = short_name.cal_chksum()?;
            for ch in long_name.encode_utf16() {
                let j = i % 13;
                let tmp;
                let ch_bytes = ch.to_le_bytes();
                if j < 5 {
                    tmp = dir::LDIR_NAME1 + j * 2;
                } else if j < 11 {
                    tmp = dir::LDIR_NAME2 + (j - 5) * 2;
                } else {
                    tmp = dir::LDIR_NAME3 + (j - 11) * 2;
                }
                buf[tmp + 0] = ch_bytes[0];
                buf[tmp + 1] = ch_bytes[1];
                // 当一个目录项的数据填充满后加入blocks中
                if tmp == 30 {
                    // 是这一项最后一个字符
                    buf[dir::LDIR_ORD] = ord;
                    buf[dir::LDIR_CHKSUM] = chksum;
                    if i < 13 {
                        // 是长目录项中的第一条
                        buf[dir::LDIR_ORD] |= 0x40;
                        buf[dir::LDIR_ATTR] = ATTR_LONG_NAME;
                        buf[dir::LDIR_TYPE] = 0;
                    }
                    blocks.push(buf.clone());
                    ord += 1;
                }
                i += 1;
            }
            // 剩余字符不足13个，无法填满长目录项
            if i % 13 > 0 {
                buf[dir::LDIR_ORD] = ord;
                if i < 13 {buf[dir::LDIR_ORD] |= 0x40}
                buf[dir::LDIR_ATTR] = ATTR_LONG_NAME;
                buf[dir::LDIR_CHKSUM] = chksum;
                // 填充剩余字符为0xFFFF
                let mut j = i % 13;
                while j < 13 {
                    let tmp;
                    if j < 5 {
                        tmp = dir::LDIR_NAME1 + j * 2;
                    } else if j < 11 {
                        tmp = dir::LDIR_NAME2 + (j - 5) * 2;
                    } else {
                        tmp = dir::LDIR_NAME3 + (j - 11) * 2;
                    }
                    buf[tmp + 0] = 0xff;
                    buf[tmp + 1] = 0xff;
                    j += 1;
                }
                blocks.push(buf.clone());
            }
        }
        blocks.reverse();
        let short_dir = ShortDir::new(
            short_name,
            &name,
            attribute,
            ctime_tenth,
            ctime,
            cdate,
            lacc_date,
            first_clus,
            wtime,
            wdate,
            file_size,
        )?;

        let buf = short_dir.to_bytes(first_clus);
        blocks.push(buf);
        let dir = ExtendInfo {
            fs: fs.clone(),

            directory_cluster: 0xffffffff,
            directory_num: 0,

            offset: 0,
            cluster_list: vec![],
        };
        Ok((dir, blocks))
    }

    fn long_name2short_name(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        long_name: &String,
    ) -> io::Result<ShortName> {
        let mut name = long_name.to_owned().to_ascii_uppercase();
        let mut flag = true; // lossy conversion flag
        name = name
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    c
                } else {
                    flag = false;
                    '_'
                }
            })
            .collect();
        let short_name: String = name.trim_start().chars().filter(|&ch| is_short_name_available_char(ch)).collect();

        let base_r = short_name.find('.').unwrap_or(short_name.len()).min(8);
        let base = &short_name[..base_r].to_uppercase();

        let ext = if base_r < short_name.len() {
            short_name[base_r..(min(base_r + 3, short_name.len()))].to_uppercase()
        } else {
            String::from("   ")
        };

        let mut short_name = [' ' as u8; 11];

        base.as_bytes()
            .iter()
            .take(8)
            .enumerate()
            .for_each(|(i, &b)| {
                short_name[i] = b;
            });
        ext.as_bytes()
            .iter()
            .take(3)
            .enumerate()
            .for_each(|(i, &b)| {
                short_name[i + 8] = b;
            });

        // 把base转换成[u8;8]类型的数组
        let mut base_arr = [b' '; 8];
        base.chars().enumerate().for_each(|(i, c)| {
            base_arr[i] = c as u8;
        });
        // 把ext转换成[u8;3]类型的数组
        let mut ext_arr = [b' '; 3];
        ext.chars().enumerate().for_each(|(i, c)| {
            ext_arr[i] = c as u8;
        });

        // 生成数字后缀
        if !flag && FatFs::check_short_name(&self.fs, &std::str::from_utf8(&short_name).unwrap().to_string()) {
            Ok(ShortName {
                base_name: base_arr,
                ext_name: ext_arr,
            })
        } else {
            let mut n: u32 = 1;
            while n <= 999999 {
                let mut x = n;
                let mut i = min(short_name.len(), 7);
                // 将x转换成字符串右对齐保存在base_arr中
                while x != 0 && i > 1 {
                    let c = (x % 10) as u8 + b'0';
                    base_arr[i] = c;
                    x /= 10;
                    i -= 1;
                }
                // 在数字前加上'~'
                base_arr[i] = b'~';

                let tmp = match std::str::from_utf8(&short_name) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Invalid data {:?}", e),
                        ));
                    }
                };
                match self.search_directory(disk, tmp) {
                    Ok(_) => {}
                    Err(_) => {
                        return Ok(ShortName {
                            base_name: base_arr,
                            ext_name: ext_arr,
                        });
                    }
                }
                n += 1;
            }
            return if n == 1000000 {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "File name collision",
                ))
            } else {
                Ok(ShortName {
                    base_name: [b' '; 8],
                    ext_name: [b' '; 3],
                })
            };
        }
    }

}

const DIR_BLOCK_SIZE: usize = 0x20;

const ATTR_READ_ONLY: u8 = 0x01;
const _ATTR_HIDDEN: u8 = 0x02;
const _ATTR_SYSTEM: u8 = 0x04;
const _ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = 0x0f;

const BASE_L: u8 = 0x08;
const EXT_L: u8 = 0x10;

impl FatFs {
    pub fn new() -> Self {
        Self {
            fat_size: 0,
            tot_sec: 0,
            data_sec: 0,
            fat_start: 0,
            data_start: 0,
            sec_per_clus: 0,
            bytes_per_sec: SECTOR_SIZE,
            max_clus: 0,
            bytes_per_clus: 0,
            fs_type: FatFsType::FAT32,
            bpb: BPB::new_empty(),
        }
    }

    pub fn get_root_info(fs: Arc<Self>) -> Mutex<Box<dyn FileOps>> {
        Mutex::new(Box::new(ExtendInfo {
            fs,
            directory_cluster: 0xffffffff,
            directory_num: 0,
            offset: 0,
            cluster_list: vec![ROOT_CLUSTER],
        }))
    }

    fn new_dir_entry(
        &self,
        disk: &mut Box<dyn FileHandler>,
        parent: &ExtendInfo,
    ) -> io::Result<(u32, u8)> {
        // 逐个读取表项，寻找空位
        let mut buf = [0u8; DIR_BLOCK_SIZE];
        let mut num = 0;
        
        let mut clus = parent.cluster_list[0];
        loop {
            self.read_dir_entry(disk, clus, num % 128, &mut buf)?;
            if buf[0] == 0 {
                break;
            }
            num += 1;
            if num % 128 == 0 {
                clus = match self.get_next_clus(disk, clus) {
                    Ok(ret) => {
                        if ret >= self.max_clus {
                            self.alloc_clus(disk, clus, false)?
                        } else {
                            ret
                        }
                    }
                    Err(ret) => {
                        return Err(ret);
                    }
                }
            }
        }
        Ok((clus, num))
    }

    fn read_dir_entry(
        &self,
        disk: &mut Box<dyn FileHandler>,
        clus: u32,
        num: u8,
        buf: &mut [u8; DIR_BLOCK_SIZE],
    ) -> io::Result<usize> {
        let position = self.to_byte_cnt(clus)? + num as usize * DIR_BLOCK_SIZE;
        disk.seek(position)?;
        disk.read(buf)
    }
    fn write_dir_entry(
        &self,
        disk: &mut Box<dyn FileHandler>,
        clus: u32,
        num: u8,
        buf: &mut [u8; DIR_BLOCK_SIZE],
    ) -> io::Result<usize> {
        let position = self.to_byte_cnt(clus)? + num as usize * DIR_BLOCK_SIZE;
        disk.seek(position)?;
        disk.write(buf)
    }

    fn file_range(
        &self,
        disk: &mut Box<dyn FileHandler>,
        extend_info: &mut ExtendInfo,
        start: usize,
        size: usize,
    ) -> io::Result<Vec<(usize, usize)>> {
        let mut ret: Vec<(usize, usize)> = Vec::new();

        let end = start + size;
        // 自req.offset开始size大小的数据所在的簇总数
        let clus_count =
            ceil_div(end, self.bytes_per_clus) - (start / self.bytes_per_clus);

        let mut buf = [0u8; 0x20];

        self.read_dir_entry(
            disk,
            extend_info.directory_cluster,
            extend_info.directory_num,
            &mut buf,
        )?;

        // 该文件的要访问的簇的首项
        let left = start / self.bytes_per_clus;
        // 该文件的要访问的簇的末项
        let right = left + clus_count;
        let mut offset = start; // 已处理部分在文件内的相对位置
        let mut left_size = size; // 剩余未处理的大小
        
        for i in left..right {
            // 写入大小超出文件大小
            if i >= extend_info.cluster_list.len() {
                let new = self.alloc_clus(disk, extend_info.cluster_list[i - 1], false)?;
                extend_info.cluster_list.push(new);
            }
            // 当前在访问的簇号
            let clus = extend_info.cluster_list[i];
            // 在簇内的相对位置
            let position = offset % self.bytes_per_clus;
            // 该簇中的在范围内的大小
            let length = if position == 0 {
                if position + left_size > self.bytes_per_clus {
                    self.bytes_per_clus
                } else {
                    left_size
                }
            } else {
                self.bytes_per_clus - position
            };

            let start = self.to_byte_cnt(clus)? + position;
            let end = start + length;
            offset += length;
            left_size -= length;
            ret.push((start, end));
        }
        Ok(ret)
    }

    fn check_short_name(&self, name: &String) -> bool {
        let parts: Vec<&str> = name.rsplitn(2, ".").collect();
        if self.fs_type != FatFsType::FAT32 {
            return false;
        }
        if parts.len() == 2 && (parts[1].len() >= 8 || parts[0].len() >= 3) {
            return false;
        } else if parts.len() == 1 && parts[0].len() >= 8 {
            return false;
        }
        let cap = check_fname_caps(name);
        if cap & 0x03 == 0x03 || cap & 0x0c == 0x0c {
            return false;
        }
        
        if parts.len() == 2 {
            for ch in parts[1].chars() {
                if !ch.is_ascii() {
                    return false;
                } else if !is_short_name_available_char(ch) {
                    return false;
                }
            }
            for ch in parts[0].chars() {
                if !ch.is_ascii() {
                    return false;
                } else if !is_short_name_available_char(ch) {
                    return false;
                }
            }
        } else {
            for ch in parts[0].chars() {
                if !ch.is_ascii() {
                    return false;
                } else if !is_short_name_available_char(ch) {
                    return false;
                }
            }
        }
        
        true
    }
    fn check_long_name(&self, name: &String) -> bool {
        if self.fs_type != FatFsType::FAT32 || name.len() > 255 {
            return false;
        } else {
            for ch in name.chars() {
                if ch == '+' || ch == ',' || ch == ';' || ch == '[' || ch == ']' {
                    return false;
                }
            }
        }
        true
    }
    fn check_name(&self, name: &String) -> io::Result<FileNameType> {
        if !self.check_short_name(name) {
            if self.fs_type == FatFsType::FAT32 && self.check_long_name(name) {
                Ok(FileNameType::LongName)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Illegal file name!",
                ))
            }
        } else {
            Ok(FileNameType::ShortName)
        }
    }
    
}

impl FatFs {
    fn to_sector_cnt(&self, clus: u32) -> io::Result<usize> {
        if clus < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid clus number!",
            ));
        }
        Ok((clus as usize - 2) * self.sec_per_clus + self.data_start as usize)
    }

    fn to_byte_cnt(&self, clus: u32) -> io::Result<usize> {
        Ok(self.to_sector_cnt(clus)? * self.bytes_per_sec)
    }

    // 获取下一个簇号，如没有则返回Error
    fn get_next_clus(&self, disk: &mut Box<dyn FileHandler>, clus: u32) -> io::Result<u32> {
        match self.fs_type {
            FatFsType::FAT32 => {
                let mut next = [0u8; 4];
                disk.seek(self.fat_start as usize * self.bytes_per_sec + clus as usize * 4)?;
                disk.read(&mut next)?;
                Ok(u32::from_le_bytes(next))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported FAT fs!",
            )),
        }
    }

    fn set_clus(&self, disk: &mut Box<dyn FileHandler>, clus: u32, value: u32) -> io::Result<()> {
        let n = match self.fs_type {
            FatFsType::FAT32 => 4,
            FatFsType::FAT16 => 2,
            FatFsType::FAT12 => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Unsupported FAT fs: FAT12!",
                ));
            }
        };
        let mut position;
        let fat_cnt;
        if self.bpb.ext_flags & (1 << 7) != 0 {
            position = (self.fat_start as usize + (self.bpb.ext_flags & 0x07) as usize)
                * self.bytes_per_sec
                + clus as usize * n;
            fat_cnt = 1;
        } else {
            position = self.fat_start as usize * self.bytes_per_sec + clus as usize * n;
            fat_cnt = self.bpb.num_fats;
        }
        for _ in 0..fat_cnt {
            disk.seek(position)?;
            disk.write(&mut value.to_le_bytes())?;
            position += self.fat_size as usize * self.bytes_per_sec;
        }
        Ok(())
    }

    fn get_all_clus(
        &self,
        disk: &mut Box<dyn FileHandler>,
        first_clus: u32,
    ) -> io::Result<Vec<u32>> {
        let mut clus_list = vec![first_clus];
        let mut clus = first_clus;
        loop {
            let ret = self.get_next_clus(disk, clus);
            match ret {
                Ok(c) => clus = c,
                Err(e) => {
                    return Err(e);
                }
            }
            if clus >= 0x0fff_fff8 {
                break;
            }
            clus_list.push(clus);
        }

        Ok(clus_list)
    }

    fn alloc_clus(
        &self,
        disk: &mut Box<dyn FileHandler>,
        last_clus: u32,
        is_first_clus: bool,
    ) -> io::Result<u32> {
        let mut buf = [0u8; 4];
        let start = self.fat_start as usize * self.bytes_per_sec;
        let mut i: usize = 3;
        let n = match self.fs_type {
            FatFsType::FAT32 => 4,
            FatFsType::FAT16 => 2,
            FatFsType::FAT12 => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Unsupported FAT fs: FAT12",
                ));
            }
        };

        disk.seek(start + i * n)?;
        disk.read(&mut buf)?;
        while u32::from_le_bytes(buf) != 0 {
            i += 1;
            disk.seek(start + i * n)?;
            disk.read(&mut buf)?;
        }
        self.set_clus(disk, i as u32, 0xffff_ffff)?;
        if !is_first_clus {
            self.set_clus(disk, last_clus, i as u32)?;
        }
        // 将新申请的簇内容清零
        disk.seek(self.to_byte_cnt(i as u32)?)?;
        for _ in 0..self.sec_per_clus {
            disk.write(&mut [0u8;SECTOR_SIZE])?;
        }
        Ok(i as u32)
    }

    fn free_clus(
        &self,
        disk: &mut Box<dyn FileHandler>,
        last_clus: u32,
        clus: u32,
    ) -> io::Result<()> {
        if last_clus < 3 && clus < 3 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid clus!"));
        }
        if last_clus > 2 && clus > 2 {
            self.set_clus(disk, last_clus, 0xffff_ffffu32)?;
            self.set_clus(disk, clus, 0u32)?;
        } else if clus > 2 {
            self.set_clus(disk, clus, 0u32)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot free root clus!",
            ));
        }
        Ok(())
    }
}

fn prepend_utf16_to_string(utf16_data: &[u16], s: &mut String) {
    let data: Vec<u16> = utf16_data
        .iter()
        .take_while(|&&x| x != 0xFFFF && x != 0x0000)
        .cloned()
        .collect();
    let new_str = String::from_utf16_lossy(&data);
    s.insert_str(0, &new_str);
}

impl ShortName {
    fn cal_chksum(&self) -> io::Result<u8> {
        let filename: Vec<u8> = self
            .base_name
            .iter()
            .chain(self.ext_name.iter())
            .cloned()
            .collect();

        Ok(filename.iter().fold(0u8, |sum, &ch| {
            let a = if (sum & 1) != 0 { 0x80 } else { 0 };
            sum.wrapping_shr(1).wrapping_add(a).wrapping_add(ch)
        }))
    }
}

fn check_fname_caps(name: &String) -> u8 {
    let mut ret = 0u8;
    let mut flag = 0u8;
    for ch in name.chars() {
        if ch == '.' {
            flag = 2;
            continue;
        }
        if ch.is_lowercase() {
            ret |= 0x1 << flag;
        } else if ch.is_uppercase() {
            ret |= 0x2 << flag;
        }
    }
    ret
}

fn to_fat32_date(date: &NaiveDate) -> u16 {
    let year = date.year() as u16 - 1980;
    let month = date.month() as u16;
    let day = date.day() as u16;
    (year << 9) | (month << 5) | day
}

fn to_fat32_time(time: &NaiveTime) -> u16 {
    let hour = time.hour() as u16;
    let minute = time.minute() as u16;
    let double_second = (time.second() / 2) as u16;
    (hour << 11) | (minute << 5) | double_second
}

fn to_fat32_time_tenth(time: &NaiveTime) -> u8 {
    let nanos = time.nanosecond();
    (nanos / 100_000_000) as u8
}

fn is_short_name_available_char(ch: char) -> bool {
    if ch.is_alphabetic() || ch.is_numeric() {
        true
    } else if ch == '$'
        || ch == '%'
        || ch == '\''
        || ch == '-'
        || ch == '_'
        || ch == '@'
        || ch == '`'
        || ch == '~'
        || ch == '!'
        || ch == '('
        || ch == ')'
        || ch == '{'
        || ch == '}'
        || ch == '^'
        || ch == '#'
        || ch == '&'
    {
        true
    } else {
        false
    }
}