use chrono::Local;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path, sync::Arc,
};
use std::error::Error;

use imagetool::{
    self, host_ops,
    utils::size2bytes,
    vfs::{FileNode, VFS},
};

use clap::{Parser, Subcommand};

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

    let host_file = host_ops::new(file, mode).unwrap();

    VFS::initialize(host_file)?;
    let mut vfs = VFS::instance();

    match command {
        Commands::New { size } => {
            vfs.as_mut().unwrap().handler.create(size2bytes(&size).unwrap_or(0))?;
        }
        Commands::Create { file_path } => {
            create_file(vfs.as_mut().unwrap(), file_path)?;
        }
        Commands::Delete { file_path } => {
            delete_file(vfs.as_mut().unwrap(), file_path)?;
        }
        Commands::Mkdir { dir_path } => {
            create_dir(vfs.as_mut().unwrap(), dir_path)?;
        }
        Commands::Copy { source, target, recursive } => {
            if recursive {
                copy_dir(vfs.as_mut().unwrap(), source, target)?;
            } else {
                copy_file(vfs.as_mut().unwrap(), Path::new(&source), target)?;
            }
        }
        Commands::Print { target } => {
            print_file(vfs.as_mut().unwrap(), target)?;
        }
    }

    Ok(())
}

fn create_dir(vfs: &mut VFS, dir_path: String) -> Result<(), Box<dyn Error>> {
    let time_now = Local::now();
    vfs.create_file(Path::new(&dir_path),
                   true,
                   0,
                   &time_now.date_naive(),
                   &time_now.time(),
                   &time_now.date_naive(),
                   &time_now.time(),
                   &time_now.date_naive(),
                   0)?;
    Ok(())
}

fn delete_file(vfs: &mut VFS, file_path: String) -> Result<(), Box<dyn Error>> {
    vfs.delete_file(Path::new(&file_path))?;
    Ok(())
}

fn create_file(vfs: &mut VFS, file_path: String) -> Result<Arc<FileNode>, Box<dyn Error>> {
    let time_now = Local::now();
    let node = vfs.create_file(Path::new(&file_path),
                   false,
                   0,
                   &time_now.date_naive(),
                   &time_now.time(),
                   &time_now.date_naive(),
                   &time_now.time(),
                   &time_now.date_naive(),
                   0)?;
    Ok(node)
}

fn print_file(vfs: &mut VFS, file_path: String) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; BLOCK_SIZE]; // 按8KB分块
    let node = vfs.open(Path::new(&file_path))?;
    println!("\n-----------Start Of File-----------");
    loop {
        let length = node.handler.lock().unwrap()
            .read(&mut vfs.handler, BLOCK_SIZE, &mut buf)
            .unwrap();
        print!("{0}", String::from_utf8_lossy(&buf));
        if length != 512 {
            println!("\n-----------End Of File-----------\n");
            break;
        }
    }
    Ok(())
}

fn copy_file(vfs: &mut VFS, source: &Path, target: String) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; BLOCK_SIZE]; // 按8KB分块
    let mut src_file = File::open(source)?;
    
    let mut copied = 0;
    let file_size = src_file.metadata()?.len() as usize;
    let node = match vfs.open(Path::new(&target)) {
        Ok(node) => {
            node
        }
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
    
    while (copied + BLOCK_SIZE) < file_size {
        src_file.read(&mut buf).unwrap();
        copied += node.handler.lock().unwrap()
            .write(&mut vfs.handler, BLOCK_SIZE, &mut buf)
            .unwrap();
    }
    // 不足一个块大小的部分
    src_file.read(&mut buf).unwrap();
    node.handler.lock().unwrap().write(&mut vfs.handler, file_size - copied, &mut buf)?;
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
                Ok(_) => {},
                Err(_) => {create_dir(vfs, target.clone())?},
            };
            let src = source.to_string() + "/" + &entry.file_name().into_string().unwrap();
            copy_dir(vfs, src, target.clone())?;
        } else {
            copy_file(vfs, path.as_path(), target.clone())?;
        }
    }
    Ok(())
}
