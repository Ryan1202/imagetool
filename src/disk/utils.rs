use std::ops::{Add, Div, Sub};

pub const KIB: usize = 1024;
pub const MIB: usize = 1024 * KIB;
pub const GIB: usize = 1024 * MIB;
// pub const TIB: usize = 1024 * GIB;
pub const KB: usize = 1000;
pub const MB: usize = 1000 * KB;
pub const GB: usize = 1000 * MB;
// pub const TB: usize = 1000 * GB;

pub const SECTOR_SIZE: usize = 512;

pub fn size2bytes(size: &str) -> Option<usize> {
    if let Ok(bytes) = size.parse::<usize>() {
        return Some(bytes);
    }

    let len = size.len();
    for i in (1..=3).rev() {
        if len < i {
            continue;
        }
        let (number_str, unit) = size.split_at(len - i);
        if let Ok(number) = number_str.parse::<usize>() {
            let bytes = match unit.to_lowercase().as_str() {
                "kb" => number * KB,
                "mb" => number * MB,
                "gb" => number * GB,
                // "tb" => number * TB,
                "k" | "kib" => number * KIB,
                "m" | "mib" => number * MIB,
                "g" | "gib" => number * GIB,
                // "t" | "tib" => number * TIB,
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
    for i in (1..=3).rev() {
        if len < i {
            continue;
        }
        let (number_str, unit) = input.split_at(len - i);
        if let Ok(number) = number_str.parse::<usize>() {
            let sectors = match unit.to_lowercase().as_str() {
                "kb" => number * KB / SECTOR_SIZE,
                "mb" => number * MB / SECTOR_SIZE,
                "gb" => number * GB / SECTOR_SIZE,
                // "tb" => number * TB / SECTOR_SIZE,
                "k" | "kib" => number * KIB / SECTOR_SIZE,
                "m" | "mib" => number * MIB / SECTOR_SIZE,
                "g" | "gib" => number * GIB / SECTOR_SIZE,
                // "t" | "tib" => number * TIB / SECTOR_SIZE,
                "s" | "sectors" => {
                    return Some(number);
                }
                "%" => {
                    match start_sector {
                        Some(start_sector) => {
                            let total_size = total_size.unwrap() / SECTOR_SIZE - start_sector;
                            return Some(start_sector + number * total_size / 100 );
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
