#[macro_use]
extern crate log;

mod config;
mod utility;

pub use config::Config;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// cli 和 gui 共用的核心执行逻辑
// 此处不再检查 config，要确保传入正确的值
// 进度回调为 (total, read)
// 如果进度回调都为 -1，则表示重置进度，对于 cli，重置光标到行首，对于 gui，把滚动条设置为不确定值状态
pub async fn run(config: &config::Config, progress_callback: &dyn Fn(i64, i64)) -> Result<()> {
    let _job = utility::setup_job()?;

    let mut workdir = config.dir.clone();
    if workdir.is_relative() {
        workdir = std::env::current_dir()?.join(workdir);
    }
    std::fs::create_dir_all(&workdir)?;

    let targetdir = workdir.join("pyembed_runtime");

    if config.skip_download {
        warn!("正在检查本地 Python 版本 ...");
        let v = get_local_python_version(&targetdir)?;
        info!("本地版本：{}.{}.{}", v.0, v.1, v.2);
    } else {
        if !is_empty_dir(&targetdir)? {
            return Err(format!("{} 目录非空", targetdir.display()).into());
        }

        let v;
        if config.pyver == "latest" || config.pyver.is_empty() {
            warn!("正在获取最新版本号 ...");
            v = get_latest_python_version().await?;
            info!("最新版本：{}", v);
        } else {
            v = config.pyver.clone();
            if utility::regex_find(r"^\d+\.\d+\.\d+$", &v).is_none() {
                return Err("版本号格式错误".into());
            }
            info!("指定版本：{}", v);
        }

        warn!("正在获取下载信息 ...");
        let info = get_python_download_info(&v, config.is32).await?;
        info!("下载链接：{}", info.0);
        info!("文件哈希：{}", info.1);

        let mut arch = "amd64";
        if config.is32 {
            arch = "x86";
        }
        let filename = format!("python-{}-embed-{}.zip", v, arch);
        let mainpath = workdir.join(filename);

        let mut mainfileexists = false;
        if mainpath.exists() {
            if let Ok(pyembeddata) = std::fs::read(&mainpath) {
                let hash = format!("{:x}", md5::compute(&pyembeddata));
                if hash.eq_ignore_ascii_case(&info.1) {
                    info!("文件已存在，跳过下载");
                    mainfileexists = true;
                }
            }
        }

        if !mainfileexists {
            warn!("正在下载 ...");
            let pyembeddata = download_progress(&info.0, progress_callback).await?;
            //let pyembeddata = std::fs::read(r"D:\下载\python-3.8.5-embed-amd64.zip")?;
            progress_callback(-1, -1);
            warn!("校验文件完整性 ...");
            let hash = format!("{:x}", md5::compute(&pyembeddata));
            if !hash.eq_ignore_ascii_case(&info.1) {
                info!("文件哈希不匹配");
                info!("预期：{}", info.1);
                info!("实际：{}", hash);
                return Err("文件哈希不匹配".into());
            }

            std::fs::write(&mainpath, &pyembeddata)?;
        }

        warn!("解压文件 ...");
        extract(&mainpath, &targetdir)?;
    }

    let pippath = workdir.join("get-pip.py");
    if !config.skip_download || !pippath.exists() {
        warn!("正在下载 pip ...");
        let pipdata =
            download_progress("https://bootstrap.pypa.io/get-pip.py", progress_callback).await?;
        progress_callback(-1, -1);
        //let pipdata = std::fs::read(r"D:\下载\get-pip.py")?;
        std::fs::write(&pippath, &pipdata)?;
    }

    warn!("修改 Python Path ...");
    ensure_pth(&targetdir)?;

    warn!("安装 pip ...");
    setup_pip(&targetdir, &pippath, Some(&config.pip_mirror))?;
    pip_install(&targetdir, &["pip"], Some(&config.pip_mirror))?;
    pip_install(
        &targetdir,
        &["setuptools", "wheel"],
        Some(&config.pip_mirror),
    )?;

    if config.packages.len() > 0 {
        warn!("安装依赖包 ...");
        pip_install(&targetdir, &config.packages, Some(&config.pip_mirror))?;
    }

    warn!("正在编译 ...");
    compile(&targetdir, config.optimize)?;

    info!("安装结果");
    pip_list(&targetdir)?;

    warn!("正在清理 ...");
    let keeppip = config.keep_pip
        || config
            .packages
            .iter()
            .any(|i| i == "pip" || i == "setuptools" || i == "wheel");
    cleanup(
        &targetdir,
        keeppip,
        config.keep_scripts,
        config.keep_dist_info,
    )?;

    warn!("完成！");
    Ok(())
}

fn is_empty_dir(dir: &std::path::Path) -> Result<bool> {
    if !dir.exists() {
        return Ok(true);
    }
    let dir = std::fs::read_dir(dir)?;
    if dir.count() > 0 {
        return Ok(false);
    }
    return Ok(true);
}

fn compile(dir: &std::path::Path, optimize: u8) -> Result<()> {
    _py_compile(dir, optimize)
}

fn _py_compile(dir: &std::path::Path, optimize: u8) -> Result<()> {
    let script = r#"
# encoding: utf-8

import sys
import os
import py_compile
import shutil

ROOT = sys.argv[1]
OPTIMIZE = int(sys.argv[2])
FOUND_PYD = False

def compile(dir):
    sitedir = os.path.join(ROOT, "Lib", "site-packages")
    for i in os.listdir(os.path.join(sitedir, dir)):
        fullname = os.path.join(sitedir, dir, i)
        shortname = os.path.join(dir, i)
        if os.path.isdir(fullname):
            if i.lower() == "__pycache__":
                print(f"删除：{shortname}")
                shutil.rmtree(fullname)
            elif i.lower().endswith(".dist-info"):
                print(f"跳过：{shortname}")
            else:
                compile(shortname)
        elif os.path.isfile(fullname):
            if i.lower().endswith(".py"):
                print(f"编译：{shortname}")
                try:
                    py_compile.compile(fullname, cfile=fullname + "c", dfile=shortname, doraise=True, optimize=OPTIMIZE)
                except:
                    print(f"编译失败，跳过：{shortname}")
                else:
                    os.remove(fullname)
            elif i.lower().endswith(".pyd"):
                global FOUND_PYD
                FOUND_PYD = True
        else:
            print(f"未知文件类型：{shortname}")

compile(".")
if not FOUND_PYD:
    print("==========")
    print("没有发现 .pyd 文件，site-packages 目录也许可以被打包为 zip")
"#;

    use std::io::Write;
    let mut cmd = new_python_command(dir);
    cmd.arg("-");
    cmd.arg(dir);
    cmd.arg(optimize.to_string());
    cmd.stdin(std::process::Stdio::piped());
    let mut process = cmd.spawn()?;
    process
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())?;
    let (t1, t2) = process_output_to_log(&mut process);
    let status = process.wait()?;
    t1.join().unwrap();
    t2.join().unwrap();
    if !status.success() {
        return Err(format!("编译失败 [{}]", status).into());
    }
    Ok(())
}

fn cleanup(
    dir: &std::path::Path,
    keeppip: bool,
    keepscripts: bool,
    keepdistinfo: bool,
) -> Result<()> {
    if !keeppip {
        pip_uninstall(dir, &["setuptools", "wheel", "pip"])?;
    }
    let mut rmdirs = vec![];
    if !keepscripts {
        let scripts = dir.join("Scripts");
        if scripts.exists() {
            rmdirs.push(scripts);
        }
    }
    if !keepdistinfo {
        let site = dir.join("Lib").join("site-packages");
        for i in std::fs::read_dir(site)? {
            let i = i?;
            let path = i.path();
            if path.is_dir() && i.file_name().to_string_lossy().ends_with(".dist-info") {
                rmdirs.push(path);
            }
        }
    }
    for i in rmdirs {
        info!("删除目录：{}", i.display());
        std::fs::remove_dir_all(i)?;
    }
    Ok(())
}

fn setup_pip(dir: &std::path::Path, pip: &std::path::Path, mirror: Option<&str>) -> Result<()> {
    let mut cmd = new_python_command(dir);
    cmd.arg(pip);
    cmd.args(&["--no-cache-dir", "--no-warn-script-location"]);
    if let Some(mirror) = mirror {
        if !mirror.is_empty() {
            cmd.args(&["-i", mirror]);
        }
    }
    let mut process = cmd.spawn()?;
    let (t1, t2) = process_output_to_log(&mut process);
    let status = process.wait()?;
    t1.join().unwrap();
    t2.join().unwrap();
    if !status.success() {
        return Err(format!("安装 pip 失败 [{}]", status).into());
    }
    Ok(())
}

fn ensure_pth(dir: &std::path::Path) -> Result<()> {
    let (major, minor, _) = get_local_python_version(dir)?;
    let pth = dir.join(format!("python{}{}._pth", major, minor));
    if !pth.exists() {
        return Err("pth 文件不存在".into());
    }
    let content = std::fs::read_to_string(&pth)?;
    let mut lines: Vec<&str> = content.lines().collect();
    let mut found = false;
    lines = lines
        .iter()
        .map(|line| {
            if line.starts_with('#') {
                if line.trim_start_matches('#').trim() == "import site" {
                    found = true;
                    return "import site";
                }
            } else if *line == "import site" {
                found = true;
                return line;
            }
            return line;
        })
        .collect();
    if !found {
        lines.push("import site");
    }
    std::fs::write(&pth, lines.join("\n"))?;
    Ok(())
}

fn get_local_python_version(dir: &std::path::Path) -> Result<(u8, u8, u8)> {
    let mut cmd = new_python_command(dir);
    cmd.args(&[
        "-c",
        "import sys; vi = sys.version_info; print(\"%d.%d.%d\" % (vi.major, vi.minor, vi.micro))",
    ]);
    let output = String::from_utf8(cmd.output()?.stdout)?;
    let v: Vec<&str> = output.trim().split('.').collect();
    if v.len() != 3 {
        return Err("无法获取本地 python 版本".into());
    }
    let v: Vec<u8> = v.iter().map(|i| i.parse::<u8>().unwrap()).collect();
    Ok((v[0], v[1], v[2]))
}

fn new_python_command(dir: &std::path::Path) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new(dir.join("python.exe"));
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.current_dir(dir);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
    cmd
}

fn read_to_log(
    read: impl std::io::Read + Send + 'static,
    level: log::Level,
) -> std::thread::JoinHandle<()> {
    use std::io::BufRead;
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(read);
        loop {
            let mut buf = vec![];
            let n = reader.read_until(b'\n', &mut buf).unwrap();
            if n == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&buf);
            log!(level, "{}", line.trim());
        }
    })
}

fn process_output_to_log(
    process: &mut std::process::Child,
) -> (std::thread::JoinHandle<()>, std::thread::JoinHandle<()>) {
    let stdout = process.stdout.take().unwrap();
    let stderr = process.stderr.take().unwrap();
    let t1 = read_to_log(stdout, log::Level::Info);
    let t2 = read_to_log(stderr, log::Level::Error);
    (t1, t2)
}

fn pip_install<I, S>(dir: &std::path::Path, pkgnames: I, mirror: Option<&str>) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = new_python_command(dir);
    cmd.args(&[
        "-m",
        "pip",
        "install",
        "--no-cache-dir",
        "--no-warn-script-location",
        "-U",
    ]);
    if let Some(mirror) = mirror {
        if !mirror.is_empty() {
            cmd.args(&["-i", mirror]);
        }
    }
    for i in pkgnames {
        cmd.arg(i);
    }
    let mut process = cmd.spawn()?;
    let (t1, t2) = process_output_to_log(&mut process);
    let status = process.wait()?;
    t1.join().unwrap();
    t2.join().unwrap();
    if !status.success() {
        return Err(format!("安装依赖包失败 [{}]", status).into());
    }
    Ok(())
}

fn pip_uninstall<I, S>(dir: &std::path::Path, pkgnames: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = new_python_command(dir);
    cmd.args(&["-m", "pip", "uninstall", "-y"]);
    for i in pkgnames {
        cmd.arg(i);
    }
    let mut process = cmd.spawn()?;
    let (t1, t2) = process_output_to_log(&mut process);
    let status = process.wait()?;
    t1.join().unwrap();
    t2.join().unwrap();
    if !status.success() {
        return Err(format!("卸载依赖包失败 [{}]", status).into());
    }
    Ok(())
}

fn pip_list(dir: &std::path::Path) -> Result<()> {
    let mut cmd = new_python_command(dir);
    cmd.args(&["-m", "pip", "list"]);
    cmd.args(&["--format", "columns"]);
    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;
    if !output.status.success() {
        info!("{}", stdout);
        return Err(format!("pip list 失败 [{}]", output.status).into());
    } else {
        // 从结果中过滤掉 pip、setuptools、wheel
        for i in stdout.lines().filter(|line| {
            !line.starts_with("pip ")
                && !line.starts_with("setuptools ")
                && !line.starts_with("wheel ")
        }) {
            info!("{}", i);
        }
    }
    Ok(())
}

fn extract(source: &std::path::Path, target: &std::path::Path) -> Result<()> {
    let zipfile = std::fs::File::open(source)?;
    let mut zip = zip::ZipArchive::new(zipfile)?;
    if !target.exists() {
        std::fs::create_dir_all(target)?;
    }
    for i in 0..zip.len() {
        let mut item = zip.by_index(i)?;
        let fullpath = target.join(item.name());
        if item.is_dir() {
            std::fs::create_dir_all(fullpath)?;
        } else {
            let mut file = std::fs::File::create(fullpath)?;
            std::io::copy(&mut item, &mut file)?;
        }
    }
    Ok(())
}

async fn get(url: impl reqwest::IntoUrl) -> reqwest::Result<String> {
    Ok(reqwest::get(url).await?.text().await?)
}

async fn get_latest_python_version() -> Result<String> {
    let body = get("https://www.python.org/downloads/windows/").await?;
    if let Some(caps) = utility::regex_find(r"Latest Python 3 Release - Python ([\d\.]+)", &body) {
        if let Some(ver) = caps.get(1) {
            return Ok(ver.as_str().into());
        }
    }
    Err("找不到版本号".into())
}

async fn get_python_download_info(ver: &str, is32: bool) -> Result<(String, String)> {
    let body = get(&format!(
        "https://www.python.org/downloads/release/python-{}/",
        ver.replace(".", "")
    ))
    .await?;
    let mut re = r#""([^"]*?)">Windows x86\-64 embeddable zip file.*?([a-fA-F0-9]{32})"#;
    if is32 {
        re = r#""([^"]*?)">Windows x86 embeddable zip file.*?([a-fA-F0-9]{32})"#;
    }
    if let Some(caps) = utility::regex_find(re, &body) {
        if caps.len() == 3 {
            return Ok((
                caps.get(1).unwrap().as_str().into(),
                caps.get(2).unwrap().as_str().into(),
            ));
        }
    }
    Err("找不到信息".into())
}

async fn download_progress(
    url: impl reqwest::IntoUrl,
    callback: &dyn Fn(i64, i64),
) -> Result<bytes::Bytes> {
    use bytes::BufMut;
    use futures_util::StreamExt;

    let res = reqwest::get(url).await?;
    if !res.status().is_success() {
        let code: u16 = res.status().into();
        return Err(format!("http request failed with status code {}", code).into());
    }
    let mut data = bytes::BytesMut::new();
    let mut total_size: i64 = -1;
    if let Some(len) = res.headers().get(reqwest::header::CONTENT_LENGTH) {
        total_size = len.to_str()?.parse::<i64>()?;
    }
    callback(total_size, 0);
    let mut stream = res.bytes_stream();
    while let Some(item) = stream.next().await {
        let item = item?;
        data.put(item);
        callback(total_size, data.len() as _);
    }
    Ok(data.into())
}
