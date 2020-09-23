# Python Embedded Downloader

写着玩，用 rust 写了个 python 嵌入式环境下载器。

执行的工作很简单（批处理也能搞定）：

1. 下载嵌入式的压缩包和 get-pip.py
2. 使用嵌入式 python 运行 get-pip.py 来安装 pip
3. 通过 pip 安装指定的依赖包
4. 把所有 .py 编译成 .pyc
5. 清理，删除 pip 等不需要的包，删除一些不需要的目录

相比 pyinstaller，用这种方式打包的 python 环境，运行更加高效，因为不必每次都把文件解压到临时目录再运行。

至于为什么用 rust 写，因为我闲。。

## 下载

仅限 Windows 系统。

<https://github.com/wzv5/pyembed_downloader/releases/latest>

## 用法

``` text
USAGE:
    pyembed_downloader [FLAGS] [OPTIONS] [PACKAGES]...

FLAGS:
        --32                下载 32 位版本，默认下载 64 位版本
    -h, --help              Prints help information
        --keep-dist-info    保留 dist-info 目录，删除此目录后将无法再通过 pip 管理依赖
        --keep-pip          保留 pip、setuptools、wheel 依赖包
        --keep-scripts      保留 Scripts 目录
        --skip-download     跳过下载，直接使用已有的文件
    -V, --version           Prints version information

OPTIONS:
        --dir <dir>           工作目录，默认为当前目录
        --pip-mirror <url>    通过指定 pip 镜像站下载依赖包

ARGS:
    <PACKAGES>...    要安装的 pip 依赖包
```
