extern crate bytes;
extern crate clap;
extern crate md5;
extern crate regex;
extern crate reqwest;

#[macro_use]
extern crate log;

mod native;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct ConsoleLogger;
impl log::Log for ConsoleLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        println!("{}", record.args());
    }

    fn flush(&self) {}
}
static MYLOGGER: ConsoleLogger = ConsoleLogger;

#[tokio::main]
async fn main() -> Result<()> {
    log::set_logger(&MYLOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    let matches = clap::App::new("pyembed_downloader")
        .version("0.0.5")
        .arg(
            clap::Arg::with_name("pyver")
                .long("py-ver")
                .takes_value(true)
                .value_name("ver")
                .default_value("latest")
                .help("下载指定版本的 Python，如 3.8.6"),
        )
        .arg(
            clap::Arg::with_name("32")
                .long("32")
                .help("下载 32 位版本，默认下载 64 位版本"),
        )
        .arg(
            clap::Arg::with_name("skip-download")
                .long("skip-download")
                .help("跳过下载，直接使用已有的文件"),
        )
        .arg(
            clap::Arg::with_name("dir")
                .long("dir")
                .takes_value(true)
                .help("工作目录，默认为当前目录"),
        )
        .arg(
            clap::Arg::with_name("pip-mirror")
                .long("pip-mirror")
                .takes_value(true)
                .value_name("url")
                .help("通过指定 pip 镜像站下载依赖包"),
        )
        .arg(
            clap::Arg::with_name("keep-scripts")
                .long("keep-scripts")
                .help("保留 Scripts 目录"),
        )
        .arg(
            clap::Arg::with_name("keep-dist-info")
                .long("keep-dist-info")
                .help("保留 dist-info 目录，删除此目录后将无法再通过 pip 管理依赖"),
        )
        .arg(
            clap::Arg::with_name("keep-pip")
                .long("keep-pip")
                .help("保留 pip、setuptools、wheel 依赖包"),
        )
        .arg(
            clap::Arg::with_name("PACKAGES")
                .index(1)
                .multiple(true)
                .help("要安装的 pip 依赖包"),
        )
        .get_matches();

    let currentdir = std::env::current_dir()?;
    let mut workdir =
        std::path::PathBuf::from(matches.value_of_os("dir").unwrap_or(currentdir.as_os_str()));
    if workdir.is_relative() {
        workdir = currentdir.join(workdir);
    }
    let targetdir = workdir.join("pyembed_runtime");
    let pyver = matches.value_of("pyver").unwrap();
    let is32 = matches.is_present("32");
    let skipdownload = matches.is_present("skip-download");
    let pipmirror = matches.value_of("pip-mirror");
    let keepscripts = matches.is_present("keep-scripts");
    let keepdistinfo = matches.is_present("keep-dist-info");
    let keeppip = matches.is_present("keep-pip");
    let packages: Vec<&str> = matches.values_of("PACKAGES").unwrap_or_default().collect();

    if packages.len() == 0 {
        use winapi::um::winuser;
        if native::MessageBoxW(
            "没有指定要安装的依赖包，确定要继续吗？",
            winuser::MB_ICONQUESTION | winuser::MB_OKCANCEL,
        ) != winuser::IDOK as u32
        {
            return Err("用户取消".into());
        }
    }

    native::setup_job()?;

    std::fs::create_dir_all(&workdir)?;

    let simple_progress = |total: i64, read: i64| {
        if total == -1 {
            print!("\r{}", read);
        } else {
            print!("\r{}%", 100 * read / total);
        }
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    };

    if skipdownload {
        let v = get_local_python_version(&targetdir)?;
        info!("本地版本：\t{}.{}.{}", v.0, v.1, v.2);
    } else {
        if !is_empty_dir(&targetdir)? {
            return Err(format!("{} 目录非空", targetdir.display()).into());
        }

        let v;
        if pyver == "latest" {
            v = get_latest_python_version().await?;
            info!("最新版本：\t{}", v);
        } else {
            v = pyver.to_owned();
            if regex_find(r"^\d+\.\d+\.\d+$", &v).is_none() {
                return Err("版本号格式错误".into());
            }
            info!("指定版本：\t{}", v);
        }

        let info = get_python_download_info(&v, is32).await?;
        info!("下载链接：\t{}", info.0);
        info!("文件哈希：\t{}", info.1);

        let mut arch = "amd64";
        if is32 {
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
            info!("正在下载 ...");
            let pyembeddata = download_progress(&info.0, &simple_progress).await?;
            //let pyembeddata = std::fs::read(r"D:\下载\python-3.8.5-embed-amd64.zip")?;
            print!("\r");
            info!("校验文件完整性 ...");
            let hash = format!("{:x}", md5::compute(&pyembeddata));
            if !hash.eq_ignore_ascii_case(&info.1) {
                info!("文件哈希不匹配");
                info!("预期：\t{}", info.1);
                info!("实际：\t{}", hash);
                return Err("文件哈希不匹配".into());
            }

            std::fs::write(&mainpath, &pyembeddata)?;
        }

        info!("解压文件 ...");
        extract(&mainpath, &targetdir)?;
    }

    let pippath = workdir.join("get-pip.py");
    if !skipdownload || !pippath.exists() {
        info!("正在下载 pip ...");
        let pipdata =
            download_progress("https://bootstrap.pypa.io/get-pip.py", &simple_progress).await?;
        print!("\r");
        //let pipdata = std::fs::read(r"D:\下载\get-pip.py")?;
        std::fs::write(&pippath, &pipdata)?;
    }

    info!("修改 Python Path ...");
    ensure_pth(&targetdir)?;

    info!("安装 pip ...");
    setup_pip(&targetdir, &pippath, pipmirror)?;
    pip_install(&targetdir, &["pip"], pipmirror)?;
    pip_install(&targetdir, &["setuptools", "wheel"], pipmirror)?;

    if packages.len() > 0 {
        info!("安装依赖包 ...");
        pip_install(&targetdir, &packages, pipmirror)?;
    }

    info!("编译 ...");
    compile(&targetdir)?;

    info!("安装结果");
    pip_list(&targetdir)?;

    info!("清理 ...");
    cleanup(&targetdir, keeppip, keepscripts, keepdistinfo)?;

    native::MessageBoxW("完成！", winapi::um::winuser::MB_ICONINFORMATION);
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

fn compile(dir: &std::path::Path) -> Result<()> {
    _py_compile(dir)
}

fn _py_compile(dir: &std::path::Path) -> Result<()> {
    let script = r#"
# encoding: utf-8

import sys
import os
import py_compile
import shutil

ROOT = sys.argv[1]
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
                    py_compile.compile(fullname, fullname + "c", doraise=True)
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
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut process = cmd.spawn()?;
    process
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())?;
    let stdout = process.stdout.take().unwrap();
    let stderr = process.stderr.take().unwrap();
    let t1 = read_to_log(stdout, log::Level::Info);
    let t2 = read_to_log(stderr, log::Level::Error);
    let status = process.wait()?;
    t1.join().unwrap();
    t2.join().unwrap();
    if !status.success() {
        return Err(format!("编译失败 [{}]", status).into());
    }
    Ok(())
}

fn read_to_log(
    read: impl std::io::Read + Send + 'static,
    level: log::Level,
) -> std::thread::JoinHandle<()> {
    use std::io::BufRead;
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(read);
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).unwrap();
            if n == 0 {
                break;
            }
            log!(level, "{}", line.trim());
        }
    })
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
        println!("删除目录：{}", i.display());
        std::fs::remove_dir_all(i)?;
    }
    Ok(())
}

fn setup_pip(dir: &std::path::Path, pip: &std::path::Path, mirror: Option<&str>) -> Result<()> {
    let mut cmd = new_python_command(dir);
    cmd.arg(pip);
    cmd.args(&["--no-cache-dir", "--no-warn-script-location"]);
    if let Some(mirror) = mirror {
        cmd.args(&["-i", mirror]);
    }
    let status = cmd.spawn()?.wait()?;
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
    let mut cmd = std::process::Command::new(dir.join("python.exe"));
    cmd.current_dir(dir);
    cmd
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
        cmd.args(&["-i", mirror]);
    }
    for i in pkgnames {
        cmd.arg(i);
    }
    let status = cmd.spawn()?.wait()?;
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
    let status = cmd.spawn()?.wait()?;
    if !status.success() {
        return Err(format!("卸载依赖包失败 [{}]", status).into());
    }
    Ok(())
}

fn pip_list(dir: &std::path::Path) -> Result<()> {
    let mut cmd = new_python_command(dir);
    cmd.args(&["-m", "pip", "list"]);
    let status = cmd.spawn()?.wait()?;
    if !status.success() {
        return Err(format!("pip list 失败 [{}]", status).into());
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

fn regex_find<'a>(re: &str, text: &'a str) -> Option<regex::Captures<'a>> {
    if let Ok(re) = regex::RegexBuilder::new(re)
        .dot_matches_new_line(true)
        .build()
    {
        return re.captures(text);
    }
    None
}

async fn get_latest_python_version() -> Result<String> {
    let body = get("https://www.python.org/downloads/windows/").await?;
    if let Some(caps) = regex_find(r"Latest Python 3 Release - Python ([\d\.]+)", &body) {
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
    if let Some(caps) = regex_find(re, &body) {
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
    use tokio::stream::StreamExt;

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
