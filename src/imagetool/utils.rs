use std::ops::{Add, Div, Sub};

pub const KB: u64 = 1024;
pub const MB: u64 = 1024 * KB;
pub const GB: u64 = 1024 * MB;
pub const TB: u64 = 1024 * GB;

pub const SECTOR_SIZE: usize = 512;

pub fn size2bytes(size: &str) -> Option<u64> {
    if let Ok(bytes) = size.parse::<u64>() {
        return Some(bytes);
    }

    let len = size.len();
    for i in (1..=2).rev() {
        if len < i {
            continue;
        }
        let (number_str, unit) = size.split_at(len - i);
        if let Ok(number) = number_str.parse::<u64>() {
            let bytes = match unit.to_lowercase().as_str() {
                "k" | "kb" => number * KB,
                "m" | "mb" => number * MB,
                "g" | "gb" => number * GB,
                "t" | "tb" => number * TB,
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
