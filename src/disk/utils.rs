use std::ops::{Add, Div, Sub};

pub const KIB: usize = 1024;
pub const MIB: usize = 1024 * KB;
pub const GIB: usize = 1024 * MB;
pub const TIB: usize = 1024 * GB;
pub const KB: usize = 1000;
pub const MB: usize = 1000 * KB;
pub const GB: usize = 1000 * MB;
pub const TB: usize = 1000 * GB;

pub const SECTOR_SIZE: usize = 512;

pub fn size2bytes(size: &str) -> Option<usize> {
    if let Ok(bytes) = size.parse::<usize>() {
        return Some(bytes);
    }

    let len = size.len();
    for i in (1..=2).rev() {
        if len < i {
            continue;
        }
        let (number_str, unit) = size.split_at(len - i);
        if let Ok(number) = number_str.parse::<usize>() {
            let bytes = match unit.to_lowercase().as_str() {
                "k" | "kb" => number * KB,
                "m" | "mb" => number * MB,
                "g" | "gb" => number * GB,
                "t" | "tb" => number * TB,
                "kib" => number * KIB,
                "mib" => number * MIB,
                "gib" => number * GIB,
                "tib" => number * TIB,
                _ => continue,
            };
            return Some(bytes);
        }
    }

    None
}

pub fn ceil_div<T: Add<Output = T> + Sub<Output = T> + Div<Output = T> + From<u8> + Copy>(
    a: T,
    b: T,
) -> T {
    (a + b - T::from(1)) / b
}

pub fn to_sectors(
    input: &String,
    start_sector: Option<usize>,
    total_size: Option<usize>,
) -> Option<usize> {
    if let Ok(sectors) = input.parse::<usize>() {
        return Some(sectors);
    }

    let len = input.len();
    for i in (1..=2).rev() {
        if len < i {
            continue;
        }
        let (number_str, unit) = input.split_at(len - i);
        if let Ok(number) = number_str.parse::<usize>() {
            let sectors = match unit.to_lowercase().as_str() {
                "k" | "kb" => number * KB / SECTOR_SIZE,
                "m" | "mb" => number * MB / SECTOR_SIZE,
                "g" | "gb" => number * GB / SECTOR_SIZE,
                "t" | "tb" => number * TB / SECTOR_SIZE,
                "kib" => number * KB / SECTOR_SIZE,
                "mib" => number * MB / SECTOR_SIZE,
                "gib" => number * GB / SECTOR_SIZE,
                "tib" => number * TB / SECTOR_SIZE,
                "s" | "sectors" => {
                    return Some(number);
                }
                "%" => {
                    match start_sector {
                        Some(start_sector) => {
                            let total_size = total_size.unwrap() - start_sector as usize;
                            return Some(number * total_size / 100 / SECTOR_SIZE);
                        },
                        None => {
                            return Some(number * total_size.unwrap() / 100 / SECTOR_SIZE);
                        },
                    } 
                }
                _ => continue,
            };
            match start_sector {
                Some(start_sector) => {
                    if sectors <= start_sector {
                        return Some(start_sector);
                    } else {
                        return Some(start_sector + sectors);
                    }
                }
                None => {
                    return Some(sectors);
                }
            }
        }
    }

    None
}

pub fn lba_to_chs(lba: u32, heads: u8, sectors: u8) -> [u8; 3] {
    let c = lba / (heads as u32 * sectors as u32);
    let h = (lba / sectors as u32) % heads as u32;
    let s = (lba % sectors as u32) + 1;

    let bytes: [u8; 3] = [h as u8, ((c >> 8) | s) as u8, c as u8];

    bytes
}
