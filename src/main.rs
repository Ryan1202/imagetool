use chrono::Local;
use std::{
    fs::{self, File},
    io::{self, Read},
    os::windows::fs::MetadataExt, path::Path,
};
use std::error::Error;

use imagetool::{
    self, host_ops,
    utils::size2bytes,
    vfs::{get_fs, FileNode, FileType},
};

use clap::{Parser, Subcommand};
use imagetool::host_ops::FileHandler;

const BLOCK_SIZE: usize = 8192;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(help = "A virtual disk image to operate")]
    file: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new image file
    New {
        #[arg(short, long, help = "size of disk")]
        size: String,
    },
    /// Create a file
    Create {
      #[arg(help = "path with file name")]
      file_path: String,
    },
    /// Delete a file
    Delete {
       #[arg(help = "file path")]
       file_path: String,
    },
    /// Create a directory
    Mkdir {
        #[arg(help = "dir path")]
        dir_path: String,
    },
    /// Copy file from host to image file
    Copy {
        #[arg(short, long, help = "copy directories recursively")]
        recursive: bool,
        #[arg(short, long, help = "host file")]
        source: String,
        #[arg(short, long, help = "dest file path with file name")]
        target: String,
    },
    /// Print file
    Print {
        #[arg(short, long)]
        target: String,
    },
}

// #[test]
// fn test() {
//     let file = fs::OpenOptions::new()
//         .read(true)
//         .write(true)
//         .open("test.img")
//         .unwrap();
//     let mut host_file = host_ops::new(file, host_ops::FileOpsMode::ReadOnly).unwrap();
//     let mut root = FileNode::new_root(&mut host_file).unwrap();
//     let target = "/p0/test/launch.json".to_string();
//     let mut path: Vec<&str> = target.split("/").collect();
//     while path[0] == "" {
//         path.remove(0);
//     }
//     let node = root.get_node(path[0].to_string()).unwrap();
//     path.remove(0);
//     let fs = &mut node.fs;
//     println!("open file");
//     let now = Utc::now();
//     let date = now.date_naive();
//     let time = now.time();
//     fs.create_file(
//         &mut host_file,
//         &"LongName.Extension".to_string(),
//         FileType::File,
//         0,
//         &date,
//         &time,
//         &date,
//         &time,
//         &date,
//         0,
//     )
//     .unwrap();
// }

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
    };
    let file = match mode {
        host_ops::FileOpsMode::ReadOnly => options
            .read(true)
            .open(args.file)
            .expect("Unable to open the file"),
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
                        .expect("Unable to open the file")
                } else {
                    // 如果是其他错误，继续传播错误
                    panic!("Error: {:?}", e)
                }
            }),
    };

    let mut host_file = host_ops::new(file, mode).unwrap();

    let mut root = FileNode::new_root(&mut host_file)?;
    match command {
        Commands::New { size } => {
            host_file.create(size2bytes(&size).unwrap_or(0))?;
        }
        Commands::Create { file_path } => {
            create_file(&mut host_file, &mut root, file_path)?;
        }
        Commands::Delete { file_path } => {
            delete_file(&mut host_file, &mut root, file_path)?;
        }
        Commands::Mkdir { dir_path } => {
            create_dir(&mut host_file, &mut root, dir_path)?;
        }
        Commands::Copy { source, target, recursive } => {
            if recursive {
                copy_dir(&mut host_file, &mut root, source, target)?;
            } else {
                copy_file(&mut host_file, &mut root, Path::new(&source), target)?;
            }
        }
        Commands::Print { target } => {
            print_file(&mut host_file, &mut root, target)?;
        }
    }

    Ok(())
}

fn create_dir(mut host_file: &mut Box<dyn FileHandler>, mut root: &mut FileNode, dir_path: String) -> Result<(), Box<dyn Error>> {
    let fs;
    let path;
    (path, fs) = get_fs(&mut root, dir_path)?;
    let time_now = Local::now();
    fs.create_file(&mut host_file,
                   &path,
                   FileType::Dir,
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

fn delete_file(mut host_file: &mut Box<dyn FileHandler>, mut root: &mut FileNode, file_path: String) -> Result<(), Box<dyn Error>> {
    let fs;
    let path;
    (path, fs) = get_fs(&mut root, file_path)?;
    let mut req = fs.open(&mut host_file, path, FileType::File)?;
    fs.delete_file(&mut host_file, &mut req)?;
    Ok(())
}

fn create_file(mut host_file: &mut Box<dyn FileHandler>, mut root: &mut FileNode, file_path: String) -> Result<(), Box<dyn Error>> {
    let fs;
    let path;
    (path, fs) = get_fs(&mut root, file_path)?;
    let time_now = Local::now();
    fs.create_file(&mut host_file,
                   &path,
                   FileType::File,
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

fn print_file(mut host_file: &mut Box<dyn FileHandler>, mut root: &mut FileNode, target: String) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; BLOCK_SIZE]; // 按8KB分块
    let fs;
    let path;
    (path, fs) = get_fs(&mut root, target)?;
    let mut req = fs.open(&mut host_file, path, FileType::File)?;
    println!("\n-----------Start Of File-----------");
    loop {
        let length = fs
            .read(&mut host_file, &mut req, &mut buf, BLOCK_SIZE)
            .unwrap();
        print!("{0}", String::from_utf8_lossy(&buf));
        if length != 512 {
            println!("\n-----------End Of File-----------\n");
            break;
        }
    }
    Ok(())
}

fn copy_file(mut host_file: &mut Box<dyn FileHandler>, mut root: &mut FileNode, source: &Path, target: String) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; BLOCK_SIZE]; // 按8KB分块
    let mut src_file = File::open(source)?;
    let (path, fs) = get_fs(&mut root, target)?;
    let mut copied = 0;
    let file_size = src_file.metadata()?.file_size() as usize;
    let mut req = match fs.open(&mut host_file, path.clone(), FileType::File) {
        Ok(req) => {
            req
        }
        Err(_) => {
            let time_now = Local::now();
            fs.create_file(
                &mut host_file,
                &path.to_string(),
                FileType::File,
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
    
    while (copied + BLOCK_SIZE) < file_size {
        src_file.read(&mut buf).unwrap();
        copied += fs
            .write(&mut host_file, &mut req, &mut buf, BLOCK_SIZE)
            .unwrap();
    }
    // 不足一个块大小的部分
    src_file.read(&mut buf).unwrap();
    fs.write(&mut host_file, &mut req, &mut buf, file_size - copied)?;
    Ok(())
}

fn copy_dir(mut host_file: &mut Box<dyn FileHandler>, root: &mut FileNode, source: String, target: String) -> Result<(), Box<dyn Error>> {
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
            let (path, fs) = get_fs(root, target.clone())?;
            match fs.open(&mut host_file, path, FileType::Dir) {
                Ok(_) => {},
                Err(_) => {create_dir(host_file, root, target.clone())?},
            };
            let src = source.to_string() + "/" + &entry.file_name().into_string().unwrap();
            copy_dir(host_file, root, src, target.clone())?;
        } else {
            copy_file(host_file, root, path.as_path(), target.clone())?;
        }
    }
    Ok(())
}
