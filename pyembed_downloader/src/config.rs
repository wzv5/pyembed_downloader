#[derive(Debug, Clone)]
pub struct Config {
    // 工作目录，默认为当前目录
    // 将把 python-x.x.x-embed-xxx.zip、get-pip.py 下载到此目录
    // 在此目录下创建 pyembed_runtime 目录作为最终结果
    pub dir: std::path::PathBuf,

    // 指定要下载的 Python 版本，格式为 3.8.6
    // 如果为空或 "latest"，将下载当前最新版
    pub pyver: String,

    // 是否下载 32 位
    pub is32: bool,

    // 跳过下载，用于下载后想要添加或更新依赖包
    pub skip_download: bool,

    // 通过指定镜像站下载，如果为空则不使用镜像站
    pub pip_mirror: String,

    // 保留 Scripts 目录
    pub keep_scripts: bool,

    // 保留 dist-info 目录，删除此目录后将无法再通过 pip 管理依赖
    pub keep_dist_info: bool,

    // 保留 pip、setuptools、wheel 依赖包
    pub keep_pip: bool,

    // 要安装的 pip 依赖包
    pub packages: Vec<String>,
}

impl Default for Config{
    fn default() -> Self {
        Config {
            dir: std::env::current_dir().unwrap(),
            pyver: "latest".into(),
            is32: false,
            skip_download: false,
            pip_mirror: "".into(),
            keep_scripts: false,
            keep_dist_info: false,
            keep_pip: false,
            packages: vec![],
        }
    }
}
