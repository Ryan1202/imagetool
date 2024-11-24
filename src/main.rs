use chrono::Local;
use imagetool::disk::{PartitionError, PartitionTableType};
use imagetool::fs_ops::fs_select_mbr_id;
use std::error::Error;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
    sync::Arc,
};

use imagetool::{
    self,
    host_ops::{self, FileHandler},
    mbr::{create_mbr_partition, MbrPartitionType},
    utils::size2bytes,
    vfs::{FileNode, VFS},
};

use clap::{Parser, Subcommand, ValueEnum};

const BLOCK_SIZE: usize = 16 * 1024;

#[derive(Debug)]
enum MyError {
    CreatePartitionError(PartitionError),
    OtherError(String),
}

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for MyError {}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(help = "需要操作的虚拟文件系统")]
    file: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 创建一个新的磁盘映像
    New {
        #[arg(short, long, help = "磁盘大小(e.g. 100M)")]
        size: String,
    },
    /// 创建文件
    Create {
        #[arg(help = "文件路径")]
        file_path: String,
    },
    /// 删除文件
    Delete {
        #[arg(help = "文件路径")]
        file_path: String,
    },
    /// 创建文件夹
    Mkdir {
        #[arg(help = "文件夹路径")]
        dir_path: String,
    },
    /// 从主机复制文件到虚拟磁盘
    Copy {
        #[arg(short, long, help = "递归赋值文件")]
        recursive: bool,
        #[arg(help = "源文件路径（主机文件）")]
        source: String,
        #[arg(help = "目标文件路径（虚拟磁盘文件）")]
        target: String,
    },
    /// 打印文件内容
    Print {
        #[arg(short, long)]
        target: String,
    },
    /// 格式化分区
    Format {
        #[arg(help = "需要格式化的文件系统")]
        partition_path: String,
        #[arg(help = "文件系统类型 (e.g. fat32)")]
        fs_type: String,
        #[arg(last = true, help = "文件系统特定的额外参数,使用key=value格式")]
        options: Vec<String>,
    },
    /// 对磁盘进行分区
    Partition {
        #[arg(help = "分区的类型")]
        partition_type: CmdPartitionType,
        #[arg(help = "分区的文件系统类型 (e.g. fat32)")]
        fs_type: String,
        #[arg(help = "开始位置")]
        start: String,
        #[arg(help = "结束位置")]
        end: String,
        #[arg(help = "引导程序路径")]
        bootloader: Option<String>,
    },
}

#[derive(ValueEnum, Clone)]
enum CmdPartitionType {
    Primary,
    Extended,
    Logical,
    GPT,
}

impl From<CmdPartitionType> for PartitionTableType {
    fn from(cmd_type: CmdPartitionType) -> Self {
        match cmd_type {
            CmdPartitionType::Primary | CmdPartitionType::Extended | CmdPartitionType::Logical => {
                PartitionTableType::MBR
            }
            CmdPartitionType::GPT => PartitionTableType::GPT,
        }
    }
}

impl From<CmdPartitionType> for MbrPartitionType {
    fn from(cmd_type: CmdPartitionType) -> Self {
        match cmd_type {
            CmdPartitionType::Primary => MbrPartitionType::Primary,
            CmdPartitionType::Extended => MbrPartitionType::Extended,
            CmdPartitionType::Logical => MbrPartitionType::Logical,
            _ => unreachable!(),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let command = args.command.unwrap();

    let mut options = fs::OpenOptions::new();
    let mode = match command {
        Commands::Copy { .. } => host_ops::FileOpsMode::ReadWrite,
        Commands::New { .. } => host_ops::FileOpsMode::ReadWrite,
        Commands::Print { .. } => host_ops::FileOpsMode::ReadOnly,
        Commands::Create { .. } => host_ops::FileOpsMode::ReadWrite,
        Commands::Delete { .. } => host_ops::FileOpsMode::ReadWrite,
        Commands::Mkdir { .. } => host_ops::FileOpsMode::ReadWrite,
        Commands::Format { .. } => host_ops::FileOpsMode::ReadWrite,
        Commands::Partition { .. } => host_ops::FileOpsMode::ReadWrite,
    };

    let file = match mode {
        host_ops::FileOpsMode::ReadOnly => {
            options.read(true).open(args.file).expect("打开文件失败")
        }
        host_ops::FileOpsMode::ReadWrite => options
            .read(true)
            .write(true)
            .create_new(true)
            .open(args.file.clone())
            .unwrap_or_else(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    // 如果文件已经存在，忽略错误
                    File::options()
                        .read(true)
                        .write(true)
                        .open(args.file)
                        .expect("打开文件失败")
                } else {
                    panic!("错误: {:?}", e)
                }
            }),
    };

    let host_file = host_ops::new(file, mode).unwrap();

    VFS::initialize(host_file)?;
    let mut vfs = VFS::instance();

    match command {
        Commands::New { size } => {
            vfs.as_mut()
                .unwrap()
                .handler
                .create(size2bytes(&size).unwrap_or(0) as u64)?;
        }
        Commands::Create { file_path } => {
            vfs.as_mut().unwrap().load_image()?;
            create_file(vfs.as_mut().unwrap(), file_path)?;
        }
        Commands::Delete { file_path } => {
            vfs.as_mut().unwrap().load_image()?;
            delete_file(vfs.as_mut().unwrap(), file_path)?;
        }
        Commands::Mkdir { dir_path } => {
            vfs.as_mut().unwrap().load_image()?;
            create_dir(vfs.as_mut().unwrap(), dir_path)?;
        }
        Commands::Copy {
            source,
            target,
            recursive,
        } => {
            vfs.as_mut().unwrap().load_image()?;
            if recursive {
                copy_dir(vfs.as_mut().unwrap(), source, target)?;
            } else {
                copy_file(vfs.as_mut().unwrap(), Path::new(&source), target)?;
            }
        }
        Commands::Print { target } => {
            vfs.as_mut().unwrap().load_image()?;
            print_file(vfs.as_mut().unwrap(), target)?;
        }
        Commands::Format {
            partition_path,
            fs_type,
            options,
        } => {
            vfs.as_mut().unwrap().load_image()?;
            format_partition(vfs.as_mut().unwrap(), partition_path, fs_type, options)?;
        }
        Commands::Partition {
            fs_type,
            partition_type,
            start,
            end,
            bootloader: bootloader_path,
        } => {
            partition(
                &mut vfs.as_mut().unwrap().handler,
                partition_type,
                fs_type,
                start,
                end,
                bootloader_path,
            )?;
        }
    }

    Ok(())
}

fn create_dir(vfs: &mut VFS, dir_path: String) -> Result<(), Box<dyn Error>> {
    let time_now = Local::now();
    vfs.create_file(
        Path::new(&dir_path),
        true,
        0,
        &time_now.date_naive(),
        &time_now.time(),
        &time_now.date_naive(),
        &time_now.time(),
        &time_now.date_naive(),
        0,
    )?;
    Ok(())
}

fn delete_file(vfs: &mut VFS, file_path: String) -> Result<(), Box<dyn Error>> {
    vfs.delete_file(Path::new(&file_path))?;
    Ok(())
}

fn create_file(vfs: &mut VFS, file_path: String) -> Result<Arc<FileNode>, Box<dyn Error>> {
    let time_now = Local::now();
    let node = vfs.create_file(
        Path::new(&file_path),
        false,
        0,
        &time_now.date_naive(),
        &time_now.time(),
        &time_now.date_naive(),
        &time_now.time(),
        &time_now.date_naive(),
        0,
    )?;
    Ok(node)
}

fn print_file(vfs: &mut VFS, file_path: String) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; BLOCK_SIZE]; // 按8KB分块
    let node = vfs.open(Path::new(&file_path))?;
    println!("\n-----------文件开始-----------");
    loop {
        let length = node
            .handler
            .lock()
            .unwrap()
            .read(&mut vfs.handler, BLOCK_SIZE, &mut buf)
            .unwrap();
        print!("{0}", String::from_utf8_lossy(&buf));
        if length != 512 {
            println!("\n-----------文件结束-----------\n");
            break;
        }
    }
    Ok(())
}

fn copy_file(vfs: &mut VFS, source: &Path, target: String) -> Result<(), Box<dyn Error>> {
    let mut src_file = File::open(source)?;
    let mut buf = [0u8; BLOCK_SIZE]; // 分块

    let mut copied = 0;
    let file_size = src_file.metadata()?.len() as usize;
    println!("copying {}", source.display());
    let node = match vfs.open(Path::new(&target)) {
        Ok(node) => node,
        Err(_) => {
            let time_now = Local::now();
            vfs.create_file(
                Path::new(&target),
                false,
                0,
                &time_now.date_naive(),
                &time_now.time(),
                &time_now.date_naive(),
                &time_now.time(),
                &time_now.date_naive(),
                file_size as u32,
            )?
        }
    };

    let mut handler = node.handler.lock().unwrap();
    while (copied + BLOCK_SIZE) < file_size {
        src_file.read(&mut buf).unwrap();

        copied += handler
            .write(&mut vfs.handler, BLOCK_SIZE, &mut buf)
            .unwrap();
    }
    // 不足一个块大小的部分
    src_file.read(&mut buf).unwrap();
    handler.write(&mut vfs.handler, file_size - copied, &mut buf)?;
    Ok(())
}

fn copy_dir(vfs: &mut VFS, source: String, target: String) -> Result<(), Box<dyn Error>> {
    let dir = fs::read_dir(source.clone())?;
    let source = source.trim_end_matches("/");
    let target = target.trim_end_matches("/");
    for x in dir {
        let entry = x?;
        let metadata = entry.metadata()?;
        let path = entry.path();
        let filename = entry.file_name().into_string().unwrap();
        let target = target.to_string() + "/" + &filename;
        if metadata.is_dir() {
            match vfs.open(Path::new(&target)) {
                Ok(_) => {}
                Err(_) => create_dir(vfs, target.clone())?,
            };
            let src = source.to_string() + "/" + &entry.file_name().into_string().unwrap();
            copy_dir(vfs, src, target.clone())?;
        } else {
            copy_file(vfs, path.as_path(), target.clone())?;
        }
    }
    Ok(())
}

fn format_partition(
    vfs: &mut VFS,
    partition_path: String,
    fs_type: String,
    options: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    vfs.format_partition(Path::new(&partition_path), &fs_type, options)?;
    Ok(())
}

fn partition(
    handler: &mut Box<dyn FileHandler>,
    partition_type: CmdPartitionType,
    fs_type: String,
    start: String,
    end: String,
    bootloader_path: Option<String>,
) -> Result<(), MyError> {
    let partition_table_type: PartitionTableType = partition_type.clone().into();
    match partition_table_type {
        PartitionTableType::MBR => {
            let mbr_partition_type = partition_type.into();
            let fs_type =
                fs_select_mbr_id(&fs_type).ok_or(MyError::OtherError("不支持的文件系统".into()))?;
            return create_mbr_partition(
                handler,
                mbr_partition_type,
                fs_type,
                &start,
                &end,
                bootloader_path,
            )
            .map_err(|e| MyError::CreatePartitionError(e));
        }
        PartitionTableType::GPT => {
            return Err(MyError::OtherError("暂不支持GPT分区表".into()));
        }
    }
}
