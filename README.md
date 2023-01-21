# ImageTool
## 一个磁盘映像操作工具

![stars](https://img.shields.io/github/stars/Ryan1202/imagetool.svg?logo=GitHub)
![forks](https://img.shields.io/github/forks/Ryan1202/imagetool.svg?logo=GitHub)
![license](https://img.shields.io/github/license/Ryan1202/imagetool.svg)

---

## 格式

### 命令格式

    imgtool imagepath command [source] [destinaiton]

* imagepath: 映像的路径

* command: 命令
    * copy 将主机文件复制到映像
        
        示例

            imgtool hd.img copy file.txt /p0/

    * mkdir 创建文件夹
        
        示例

            imgtool hd.img mkdir folder /p0/


* source: 部分命令使用的源文件路径

* destination: 部分命令使用的目的文件路径

### 映像路径格式

    /pN/....
N=分区号(从0开始)

---

## **须知**

* **提交代码前请使用clang-format格式化保证代码格式一致**

---
## 编译

    make

调试

    make dbg