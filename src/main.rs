extern crate bytes;
extern crate clap;
extern crate md5;
extern crate regex;
extern crate reqwest;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = clap::App::new("pyembed_downloader")
        .version("0.0.2")
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
    let targetdir = workdir.join("target");
    let is32 = matches.is_present("32");
    let skipdownload = matches.is_present("skip-download");
    let pipmirror = matches.value_of("pip-mirror");
    let keepscripts = matches.is_present("keep-scripts");
    let keepdistinfo = matches.is_present("keep-dist-info");
    let keeppip = matches.is_present("keep-pip");
    let packages: Vec<&str> = matches.values_of("PACKAGES").unwrap_or_default().collect();

    unsafe {
        setup_job()?;
    }

    std::fs::create_dir_all(&workdir)?;

    if skipdownload {
        let v = get_local_python_version(&targetdir)?;
        println!("本地版本：\t{}.{}.{}", v.0, v.1, v.2);
    } else {
        if !is_empty_dir(&targetdir)? {
            return Err(format!("{} 目录非空", targetdir.display()).into());
        }

        let v = get_latest_python_version().await?;
        println!("最新版本：\t{}", v);

        let info = get_python_download_info(&v, is32).await?;
        println!("下载链接：\t{}", info.0);
        println!("文件哈希：\t{}", info.1);

        println!("正在下载 ...");
        let pyembeddata = download(&info.0).await?;
        //let pyembeddata = std::fs::read(r"D:\下载\python-3.8.5-embed-amd64.zip")?;

        println!("校验文件完整性 ...");
        let hash = format!("{:x}", md5::compute(&pyembeddata));
        if !hash.eq_ignore_ascii_case(&info.1) {
            println!("文件哈希不匹配");
            println!("预期：\t{}", info.1);
            println!("实际：\t{}", hash);
            return Err("文件哈希不匹配".into());
        }

        let mut arch = "amd64";
        if is32 {
            arch = "x86";
        }
        let filename = format!("python-{}-embed-{}.zip", v, arch);
        let mainpath = workdir.join(filename);
        std::fs::write(&mainpath, &pyembeddata)?;

        println!("解压文件 ...");
        extract(&mainpath, &targetdir)?;
    }

    let pippath = workdir.join("get-pip.py");
    if !skipdownload || !pippath.exists() {
        println!("正在下载 pip ...");
        let pipdata = download("https://bootstrap.pypa.io/get-pip.py").await?;
        //let pipdata = std::fs::read(r"D:\下载\get-pip.py")?;
        std::fs::write(&pippath, &pipdata)?;
    }

    println!("修改 Python Path ...");
    ensure_pth(&targetdir)?;

    println!("安装 pip ...");
    setup_pip(&targetdir, &pippath, pipmirror)?;
    pip_install(&targetdir, &["pip"], pipmirror)?;
    pip_install(&targetdir, &["setuptools", "wheel"], pipmirror)?;

    if packages.len() > 0 {
        println!("安装依赖包 ...");
        pip_install(&targetdir, &packages, pipmirror)?;
    }

    println!("编译 ...");
    compile(&targetdir)?;

    println!("安装结果");
    pip_list(&targetdir)?;

    println!("清理 ...");
    cleanup(&targetdir, keeppip, keepscripts, keepdistinfo)?;

    println!("完成！");
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
    let script = format!(
        r#"
# encoding: utf-8

ROOT = r"{}"
FOUND_PYD = False

import os
import py_compile
import shutil

def compile(dir):
    sitedir = os.path.join(ROOT, "Lib", "site-packages")
    for i in os.listdir(os.path.join(sitedir, dir)):
        fullname = os.path.join(sitedir, dir, i)
        shortname = os.path.join(dir, i)
        if os.path.isdir(fullname):
            if i.lower() == "__pycache__":
                print(f"删除：{{shortname}}")
                shutil.rmtree(fullname)
            elif i.lower().endswith(".dist-info"):
                print(f"跳过：{{shortname}}")
            else:
                compile(shortname)
        elif os.path.isfile(fullname):
            if i.lower().endswith(".py"):
                print(f"编译：{{shortname}}")
                try:
                    py_compile.compile(fullname, fullname + "c", doraise=True)
                except:
                    print(f"编译失败，跳过：{{shortname}}")
                else:
                    os.remove(fullname)
            elif i.lower().endswith(".pyd"):
                global FOUND_PYD
                FOUND_PYD = True
        else:
            print(f"未知文件类型：{{shortname}}")

compile(".")
if not FOUND_PYD:
    print("==========")
    print("没有发现 .pyd 文件，site-packages 目录也许可以被打包为 zip")
"#,
        dir.to_str().unwrap()
    );

    use std::io::Write;
    let mut cmd = new_python_command(dir);
    cmd.arg("-");
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.stdin(std::process::Stdio::piped());
    let mut process = cmd.spawn()?;
    process
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())?;
    let status = process.wait()?;
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

async fn download(url: impl reqwest::IntoUrl) -> reqwest::Result<bytes::Bytes> {
    Ok(reqwest::get(url).await?.bytes().await?)
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

// https://github.com/rust-lang/cargo/blob/master/src/cargo/util/job.rs
// 简单一抄，凑合能用
unsafe fn setup_job() -> Result<()> {
    use winapi::shared::minwindef::*;
    use winapi::um::jobapi2::*;
    use winapi::um::processthreadsapi::*;
    use winapi::um::winnt::*;

    let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
    if job.is_null() {
        return Err("CreateJobObject failed".into());
    }
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
    info = std::mem::zeroed();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let r = SetInformationJobObject(
        job,
        JobObjectExtendedLimitInformation,
        &mut info as *mut _ as LPVOID,
        std::mem::size_of_val(&info) as DWORD,
    );
    if r == 0 {
        return Err("SetInformationJobObject failed".into());
    }
    let me = GetCurrentProcess();
    let r = AssignProcessToJobObject(job, me);
    if r == 0 {
        return Err("AssignProcessToJobObject failed".into());
    }
    Ok(())
}
