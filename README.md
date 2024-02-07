# ImageTool
## 一个磁盘映像操作工具

![stars](https://img.shields.io/github/stars/Ryan1202/imagetool.svg?logo=GitHub)
![forks](https://img.shields.io/github/forks/Ryan1202/imagetool.svg?logo=GitHub)
![license](https://img.shields.io/github/license/Ryan1202/imagetool.svg)
![release](https://img.shields.io/github/release/Ryan1202/imagetool.svg)
---

注意:写入大于分区剩余空间大小的文件可能会导致分区损坏

## 格式

### 命令格式

    imagetool.exe <FILE> [COMMAND]

* FILE: 映像的路径

* COMMAND: 命令
    * new 创建映像
      
          Usage: imagetool.exe <FILE> new --size <SIZE>

          Options:
          -s, --size <SIZE>  size of disk
          -h, --help         Print help

      示例

            imgtool hd.img new 1474560 #创建1.44MB大小的磁盘映像
    * create 创建文件
    
          Usage: imagetool.exe <FILE> create <FILE_PATH>

          Arguments:
          <FILE_PATH>  path with file name

      示例

            imgtool hd.img create /p0/text.txt #在第一个分区中创建文件text.txt
    * delete 删除文件

          Usage: imagetool.exe <FILE> delete <FILE_PATH>

          Arguments:
          <FILE_PATH>  file path

      示例

          imgtool hd.img delete /p0/text.txt #在第一个分区中删除文件text.txt
    * copy 将主机文件复制到映像

          Usage: imagetool.exe <FILE> new --size <SIZE>

          Options:
          -s, --size <SIZE>  size of disk
          -h, --help         Print help

        示例

            imgtool hd.img copy file.txt /p0/file.txt

    * mkdir 创建文件夹

          Usage: imagetool.exe <FILE> new --size <SIZE>

          Options:
          -s, --size <SIZE>  size of disk
          -h, --help         Print help

        示例

            imgtool hd.img mkdir folder /p0/dir #在第一个分区中创建文件夹dir

    * copydir 复制文件夹下的所有文件和子文件夹

      <span style="color:red">*该功能暂不可用*</span>
        
        示例

            imgtool hd.img copydir folder/ /p0/


### 映像路径格式

    /pN/....
N=分区号(从0开始)
