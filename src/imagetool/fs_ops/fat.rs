use bincode::deserialize;
use byteorder::{ByteOrder, LittleEndian};
use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::cmp::min;
use std::{io, vec};

use self::dir::{
    DIR_ATTR, DIR_CRT_DATE, DIR_CRT_TIME, DIR_CRT_TIME_TENTH, DIR_FILE_SIZE, DIR_FST_CLUS_HI,
    DIR_FST_CLUS_LO, DIR_LST_ACC_DATE, DIR_NAME, DIR_NTRES, DIR_WRT_DATE, DIR_WRT_TIME,
};

use super::{FileSystem, Request};
use crate::host_ops::FileHandler;
use crate::imagetool::mem_pool::{CallBack, Empty, Pool};
use crate::utils::{ceil_div, SECTOR_SIZE};
use crate::vfs::{FileType, PtPosition};

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
    pub(crate) const LDIR_ORD: usize = 0;

    pub(crate) const LDIR_NAME1: usize = 1;

    pub(crate) const LDIR_ATTR: usize = 11;

    pub(crate) const LDIR_TYPE: usize = 12;

    pub(crate) const LDIR_CHKSUM: usize = 13;

    pub(crate) const LDIR_NAME2: usize = 14;

    pub(crate) const _LDIR_FST_CLUS_LO: usize = 26;

    pub(crate) const LDIR_NAME3: usize = 28;

    pub(crate) const DIR_NAME: usize = 0;

    pub(crate) const DIR_ATTR: usize = 11;

    pub(crate) const DIR_NTRES: usize = 12;

    pub(crate) const DIR_CRT_TIME_TENTH: usize = 13;

    pub(crate) const DIR_CRT_TIME: usize = 14;

    pub(crate) const DIR_CRT_DATE: usize = 16;

    pub(crate) const DIR_LST_ACC_DATE: usize = 18;

    pub(crate) const DIR_FST_CLUS_HI: usize = 20;

    pub(crate) const DIR_WRT_TIME: usize = 22;

    pub(crate) const DIR_WRT_DATE: usize = 24;

    pub(crate) const DIR_FST_CLUS_LO: usize = 26;

    pub(crate) const DIR_FILE_SIZE: usize = 28;
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
    /// 分区的根目录
    root: DirInfo,
    /// 文件树缓存
    cache: Pool<DirInfo>,
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

#[derive(Clone, Debug)]
struct DirInfo {
    name: String,
    ftype: FileType,

    parent: usize,
    idx: usize,
    children: Vec<usize>,
    offset: u32,
    clus_list: Vec<u32>,
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
}

impl Empty for DirInfo {
    fn new_empty() -> Self {
        Self {
            name: "".to_string(),
            ftype: FileType::File,
            parent: 0,
            idx: 0,
            children: Vec::new(),
            offset: 0,
            clus_list: vec![],
        }
    }
    fn is_empty(&self) -> bool {
        if self.offset == 0 {
            true
        } else {
            false
        }
    }
}

impl CallBack for DirInfo {
    fn set_index(&mut self, idx: usize) -> Self {
        self.idx = idx;
        self.clone()
    }
}

impl FileSystem for FatFs {
    fn init(&mut self, disk: &mut Box<dyn FileHandler>, pos: &PtPosition) -> io::Result<()> {
        let mut buf = [0u8; SECTOR_SIZE];
        disk.seek(pos.start as usize * SECTOR_SIZE);
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

        let root_clus = vec![2];
        let root_dir = DirInfo {
            name: "root".to_string(),
            ftype: FileType::Dir,
            parent: 0,
            idx: 0,
            children: vec![],
            offset: 0,
            clus_list: root_clus,
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
        self.root = root_dir;
        self.bpb = bpb;

        self.cache.append(&mut self.root);
        Ok(())
    }

    fn open(&mut self, disk: &mut Box<dyn FileHandler>, path: String) -> io::Result<Request> {
        let dir_idx = self.get_parent_dir(disk, &path, FileType::File)?;
        let dir = self.cache.read(dir_idx);

        Ok(Request {
            idx: dir.idx,
            offset: 0,
        })
    }

    fn read(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        req: &mut Request,
        buf: &mut [u8],
        size: usize,
    )
        -> io::Result<usize> {
        let range = self.file_range(disk, req, size)?;
        let mut done = 0;
        for (start, end) in range {
            let length = end - start;
            let mut tmp_buf = Vec::with_capacity(length);
            tmp_buf.resize(length, 0);
            disk.seek(start);
            disk.read(&mut tmp_buf)?;
            buf[done..done + length].copy_from_slice(&tmp_buf);
            done += length;
        }
        req.offset += done;
        Ok(done)
    }

    fn write(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        req: &mut Request,
        buf: &mut [u8],
        size: usize,
    )
        -> io::Result<usize> {
        let range = self.file_range(disk, req, size)?;
        let mut done = 0;
        for (start, end) in range {
            let length = end - start;
            disk.seek(start);
            disk.write(buf)?;
            done += length;
        }
        Ok(done)
    }

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
    )
        -> io::Result<Request> {
        let cdate = to_fat32_date(create_date);
        let ctime = to_fat32_time(create_time);
        let wdate = to_fat32_date(write_date);
        let wtime = to_fat32_time(write_time);
        let lacc_date = to_fat32_date(last_acc_date);
        let ctime_tenth = to_fat32_time_tenth(create_time);
        let attribute = if attr & 0b011_011_011 == 0b011_011_011 {
            ATTR_READ_ONLY
        } else {
            0
        } | match ftype {
            FileType::File | FileType::Link => ATTR_ARCHIVE,
            FileType::Dir => ATTR_ARCHIVE | ATTR_DIRECTORY,
        };
        let first_clus = self.alloc_clus(disk, 0, true)?;

        let mut path_split: Vec<&str> = path.split('/').collect();
        let path_split = path_split.join("/");
        let parent_idx = self.get_parent_dir(disk, &path_split, FileType::Dir)?;

        let (mut dir, blocks) = self.create_file_block(
            disk,
            parent_idx,
            path,
            ftype,
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
        let mut clus;
        let mut num;
        for mut i in blocks {
            (clus, num) = self.new_dir_entry(disk, parent_idx as u32)?;
            self.write_dir_entry(disk, clus, num, &mut i)?;
        }
        dir.clus_list.push(first_clus);
        dir.idx = self.cache.append(&mut dir);
        Ok(Request {
            idx: dir.idx,
            offset: dir.offset as usize,
        })
    }

    fn delete_file(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        req: &mut Request,
    )
        -> io::Result<()> {
        let this = self.cache.read(req.idx);
        let parent = self.cache.read(this.parent);
        let clus_num = this.offset as usize / (self.bytes_per_clus / 32);
        let mut buf = [0u8; 32];
        // 读取文件对应的表项
        self.read_dir_entry(
            disk,
            parent.clus_list[clus_num],
            this.offset as usize,
            &mut buf,
        )?;
        // 标记为已删除
        buf[0] = 0xe5;
        // 写入
        self.write_dir_entry(
            disk,
            parent.clus_list[clus_num],
            this.offset as usize,
            &mut buf,
        )?;
        Ok(())
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
    pub fn new_empty() -> Self {
        Self {
            fat_size: 0,
            tot_sec: 0,
            data_sec: 0,
            fat_start: 0,
            data_start: 0,
            sec_per_clus: 0,
            bytes_per_sec: SECTOR_SIZE,
            max_clus: 0,
            root: DirInfo::new_empty(),
            bytes_per_clus: 0,
            cache: Pool::new(),
            fs_type: FatFsType::FAT32,
            bpb: BPB::new_empty(),
        }
    }

    fn new_dir_entry(
        &self,
        disk: &mut Box<dyn FileHandler>,
        parent_idx: u32,
    ) -> io::Result<(u32, usize)> {
        // 逐个读取表项，寻找空位
        let mut buf = [0u8; DIR_BLOCK_SIZE];
        let mut num = 0;
        let parent_dir = self.cache.read(parent_idx as usize);
        let mut clus = parent_dir.clus_list[0];
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
        num: usize,
        buf: &mut [u8; DIR_BLOCK_SIZE],
    ) -> io::Result<usize> {
        let position = self.to_byte_cnt(clus)? + num * DIR_BLOCK_SIZE;
        disk.seek(position);
        disk.read(buf)
    }
    fn write_dir_entry(
        &self,
        disk: &mut Box<dyn FileHandler>,
        clus: u32,
        num: usize,
        buf: &mut [u8; DIR_BLOCK_SIZE],
    ) -> io::Result<usize> {
        let position = self.to_byte_cnt(clus)? + num * DIR_BLOCK_SIZE;
        disk.seek(position);
        disk.write(buf)
    }

    fn search_in_dir(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        dir: &mut DirInfo,
        name: &str,
    ) -> io::Result<usize> {
        let mut clus_i = 0;
        let clus = &dir.clus_list;

        let mut buf = [0u8; 0x20];
        let mut i: i32 = -1;
        let mut flag: bool = false;
        let mut result: Option<[u8; 32]> = None;
        let len = dbg!(name).len();
        let mut j;

        let mut new = DirInfo {
            name: "".to_string(),
            ftype: FileType::Dir,
            parent: 0,
            idx: 0,
            children: vec![],
            offset: 0,
            clus_list: vec![],
        };

        'outer: loop {
            i += 1;
            if i as usize >= clus.len() * (self.bytes_per_clus / 0x20) {
                break;
            }
            self.read_dir_entry(disk, clus[clus_i], i as usize, &mut buf)?;
            if buf[0] == 0xe5 || buf[0] == 0x00 || buf[0] == 0x05 {
                continue;
            }
            let mut ldir: LongDir = deserialize(&buf).unwrap();

            let mut fname = String::new();
            if let FatFsType::FAT32 = self.fs_type {
                let chksum = ldir.chksum;
                while ldir.attr == ATTR_LONG_NAME && ldir.ord != 0xe5 {
                    prepend_utf16_to_string(&ldir.name3, &mut fname);
                    prepend_utf16_to_string(&ldir.name2, &mut fname);
                    prepend_utf16_to_string(&ldir.name1, &mut fname);

                    i += 1;
                    if i as u32 > (self.bytes_per_clus as u32 / 0x20) {
                        clus_i += 1;
                        i = 0;
                    }
                    self.read_dir_entry(disk, clus[clus_i], i as usize, &mut buf)?;
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
                    new.name = fname;
                    result = Some(buf);
                    break 'outer;
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Unsupported FAT fs!",
                ));
            }

            if name.is_ascii() {
                j = 0;
                let name_bytes = name.as_bytes();
                flag = true;
                // 匹配文件名
                for &x in buf[..8].into_iter() {
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
                        j += 1;
                    }
                    if flag {
                        j += 1;
                        continue;
                    } else {
                        continue 'outer;
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
                            if x != name_bytes[j] {
                                flag = false;
                            }
                        }
                        if flag {
                            j += 1;
                            continue;
                        } else {
                            continue 'outer;
                        }
                    }
                }
                flag = true;
                result = Some(buf);
                break 'outer;
            }
        }
        if flag == true {
            match result {
                Some(sdir) => {
                    let clus = ((dbg!(LittleEndian::read_u16(&sdir[20..22])) as u32) << 16)
                        | (dbg!(LittleEndian::read_u16(&sdir[26..28])) as u32);

                    new.parent = dir.idx;
                    new.offset = i as u32;
                    new.clus_list = self.get_all_clus(disk, clus)?;
                    let idx = self.cache.append(&mut new);
                    dir.children.push(idx);
                    self.cache.update(dir.idx, dir);
                    Ok(idx)
                }
                None => Err(io::Error::new(io::ErrorKind::NotFound, "find dir failed")),
            }
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "find dir failed"))
        }
    }

    fn file_range(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        req: &Request,
        size: usize,
    ) -> io::Result<Vec<(usize, usize)>> {
        let mut ret: Vec<(usize, usize)> = Vec::new();

        let mut dir = self.cache.read(req.idx);
        // 自req.offset开始size大小的数据所在的簇总数
        let clus_count =
            ceil_div(req.offset + size, self.bytes_per_clus) - (req.offset / self.bytes_per_clus);

        let mut buf = [0u8; 0x20];
        let dir_clus = self.cache.read(dir.parent); // 该目录项的父目录
        let dir_per_clus = self.bytes_per_clus / 0x20; // 每簇目录项总数

        self.read_dir_entry(
            disk,
            dir_clus.clus_list[dir.offset as usize / dir_per_clus],
            dir.offset as usize % dir_per_clus,
            &mut buf,
        )?;
        // 文件大小
        let fsize = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]) as usize;
        // 文件的簇总数
        let clus_len = ceil_div(fsize, self.bytes_per_clus);

        // 该文件的要访问的簇的首项
        let left = req.offset / self.bytes_per_clus;
        // 该文件的要访问的簇的末项
        let right = if clus_len >= left + clus_count {
            left + clus_count
        } else {
            clus_len
        };
        let mut offset = req.offset; // 已处理部分在文件内的相对位置
        let mut left_size = size; // 剩余未处理的大小
        let mut updated = false; // 是否更新过
        for i in left..right {
            // 写入大小超出文件大小
            if i >= dir.clus_list.len() {
                updated = true;
                let new = self.alloc_clus(disk, dir.clus_list[i - 1], false)?;
                dir.clus_list.push(new);
            }
            // 当前在访问的簇号
            let clus = dir.clus_list[i];
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
        if updated {
            self.cache.update(dir.idx, &mut dir);
        }
        Ok(ret)
    }

    fn check_short_name(&self, name: &String) -> bool {
        if name.len() > 11 && self.fs_type != FatFsType::FAT32 {
            return false;
        }
        let cap = check_fname_caps(name);
        if cap & 0x03 == 0x03 || cap & 0x0c == 0x0c {
            return false;
        }
        let mut index = 0;
        let mut flag = true;
        for ch in name.chars() {
            if !ch.is_ascii() {
                return false;
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
                return false;
            }
            if index >= 11 {
                return false;
            }
            if flag {
                if ch == '.' {
                    index = 7;
                    flag = false;
                } else if index >= 7 {
                    return false;
                }
            } else {
                if ch == '.' {
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
    fn long_name2short_name(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        dir: &mut DirInfo,
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
        let _ = name.trim_start();

        let base_r = name.find('.').unwrap_or(name.len()).min(8);
        let base = &name[..base_r].to_uppercase();

        let ext = if base_r < name.len() {
            name[base_r + 1..min(base_r + 4, name.len())].to_uppercase()
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
        if !flag && self.check_short_name(&std::str::from_utf8(&short_name).unwrap().to_string()) {
            Ok(ShortName {
                base_name: base_arr,
                ext_name: ext_arr,
            })
        } else {
            let mut n: u32 = 1;
            while n <= 999999 {
                let mut x = n;
                let mut i = 7;
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
                match self.search_in_dir(disk, dir, tmp) {
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

    fn create_file_block(
        &mut self,
        disk: &mut Box<dyn FileHandler>,
        parent_idx: usize,
        name: &String,
        ftype: FileType,
        attribute: u8,
        ctime_tenth: u8,
        ctime: u16,
        cdate: u16,
        lacc_date: u16,
        first_clus: u32,
        wtime: u16,
        wdate: u16,
        file_size: u32,
    ) -> io::Result<(DirInfo, Vec<[u8; 32]>)> {
        let mut parent_dir = self.cache.read(parent_idx);
        let name_type = self.check_name(name)?;
        let mut blocks: Vec<[u8; 32]> = Vec::new();
        let short_name = match name_type {
            FileNameType::LongName => self.long_name2short_name(disk, &mut parent_dir, name)?,
            FileNameType::ShortName => ShortName::new(name, &self)?,
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

        let mut buf = [0u8; 32];
        for (i, &ch) in short_dir.name.base_name.iter().enumerate() {
            buf[DIR_NAME + i] = ch;
        }
        for (i, &ch) in short_dir.name.ext_name.iter().enumerate() {
            buf[DIR_NAME + 8 + i] = ch;
        }
        buf[DIR_ATTR] = short_dir.attr;
        buf[DIR_NTRES] = short_dir.ntres;
        buf[DIR_CRT_TIME_TENTH] = short_dir.create_time_tenth;
        buf[DIR_CRT_TIME + 0] = short_dir.create_time as u8;
        buf[DIR_CRT_TIME + 1] = (short_dir.create_time >> 8) as u8;
        buf[DIR_CRT_DATE + 0] = short_dir.create_date as u8;
        buf[DIR_CRT_DATE + 1] = (short_dir.create_date >> 8) as u8;
        buf[DIR_LST_ACC_DATE + 0] = short_dir.last_acc_date as u8;
        buf[DIR_LST_ACC_DATE + 1] = (short_dir.last_acc_date >> 8) as u8;
        buf[DIR_FST_CLUS_LO + 0] = first_clus as u8;
        buf[DIR_FST_CLUS_LO + 1] = (first_clus >> 8) as u8;
        buf[DIR_FST_CLUS_HI + 0] = (first_clus >> 16) as u8;
        buf[DIR_FST_CLUS_HI + 1] = (first_clus >> 24) as u8;
        buf[DIR_WRT_TIME + 0] = short_dir.write_time as u8;
        buf[DIR_WRT_TIME + 1] = (short_dir.write_time >> 8) as u8;
        buf[DIR_WRT_DATE + 0] = short_dir.write_date as u8;
        buf[DIR_WRT_DATE + 1] = (short_dir.write_date >> 8) as u8;
        buf[DIR_FILE_SIZE + 0] = short_dir.file_size as u8;
        buf[DIR_FILE_SIZE + 1] = (short_dir.file_size >> 8) as u8;
        buf[DIR_FILE_SIZE + 2] = (short_dir.file_size >> 16) as u8;
        buf[DIR_FILE_SIZE + 3] = (short_dir.file_size >> 24) as u8;

        blocks.push(buf);
        let dir = DirInfo {
            name: name.to_string(),
            ftype,
            parent: parent_idx,
            idx: 0,
            children: Vec::new(),
            offset: 0,
            clus_list: vec![],
        };
        Ok((dir, blocks))
    }

    fn get_parent_dir(&mut self, disk: &mut Box<dyn FileHandler>, path: &String, file_type: FileType) -> io::Result<(usize)> {
        let mut names: Vec<&str> = path.split('/').collect();
        let mut idx = self.root.idx;
        let mut dir = self.cache.read(idx);

        while names[0] == "" {
            names.pop();
        }

        for dir_name in names {
            dbg!(&dir.name, idx, &dir);
            let children = &dir.children;
            if !children.is_empty() && dir.ftype != file_type {
                for &index in children {
                    let tmp = self.cache.read(index);
                    if tmp.name == dir_name {
                        idx = index;
                    }
                }
            } else {
                idx = self.search_in_dir(disk, &mut dir, dir_name)?;
            }
            dir = self.cache.read(idx);
        }
        Ok(idx)
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
                disk.seek(self.fat_start as usize * self.bytes_per_sec + clus as usize * 4);
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
            disk.seek(position);
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

        disk.seek(start + i * n);
        disk.read(&mut buf)?;
        while u32::from_le_bytes(buf) != 0 {
            i += 1;
            disk.seek(start + i * n);
            disk.read(&mut buf)?;
        }
        self.set_clus(disk, i as u32, 0xffff_ffff)?;
        if !is_first_clus {
            self.set_clus(disk, last_clus, i as u32)?;
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
