extern crate log;

use pyembed_downloader::{run, Config, Result};

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
    let matches = clap::Command::new("pyembed_downloader")
        .version(clap::crate_version!())
        .arg(
            clap::Arg::new("pyver")
                .long("py-ver")
                .num_args(1)
                .value_name("ver")
                .help("下载指定版本的 Python，如 3.8.6"),
        )
        .arg(
            clap::Arg::new("32")
                .long("32")
                .num_args(0)
                .help("下载 32 位版本，默认下载 64 位版本"),
        )
        .arg(
            clap::Arg::new("skip-download")
                .long("skip-download")
                .num_args(0)
                .help("跳过下载，直接使用已有的文件"),
        )
        .arg(
            clap::Arg::new("dir")
                .long("dir")
                .num_args(1)
                .help("工作目录，默认为 <当前目录>\\pyembed_runtime\\"),
        )
        .arg(
            clap::Arg::new("cachedir")
                .long("cache-dir")
                .num_args(1)
                .help("缓存目录，默认为当前目录"),
        )
        .arg(
            clap::Arg::new("python-mirror")
                .long("python-mirror")
                .num_args(1)
                .value_name("url")
                .help("通过指定的镜像站下载 Python 安装包"),
        )
        .arg(
            clap::Arg::new("pip-mirror")
                .long("pip-mirror")
                .num_args(1)
                .value_name("url")
                .help("通过指定 pip 镜像站下载依赖包"),
        )
        .arg(
            clap::Arg::new("keep-scripts")
                .long("keep-scripts")
                .num_args(0)
                .help("保留 Scripts 目录"),
        )
        .arg(
            clap::Arg::new("keep-dist-info")
                .long("keep-dist-info")
                .num_args(0)
                .help("保留 dist-info 目录，删除此目录后将无法再通过 pip 管理依赖"),
        )
        .arg(
            clap::Arg::new("keep-pip")
                .long("keep-pip")
                .num_args(0)
                .help("保留 pip、setuptools、wheel 依赖包"),
        )
        .arg(
            clap::Arg::new("optimize")
                .long("optimize")
                .num_args(1)
                .value_name("level")
                .help(
                    "优化编译级别：0（不优化），1（删除断言，关闭调试），2（同时删除文档字符串）",
                ),
        )
        .arg(
            clap::Arg::new("PACKAGES")
                .index(1)
                .num_args(0..)
                .help("要安装的 pip 依赖包"),
        )
        .get_matches();
    let mut config = Config::default();
    if let Some(mut s) = matches.get_raw("dir") {
        let mut p = std::path::PathBuf::from(s.next().unwrap());
        if p.is_relative() {
            p = std::env::current_dir()?.join(p);
        }
        config.dir = p;
    }
    if let Some(mut s) = matches.get_raw("cachedir") {
        let mut p = std::path::PathBuf::from(s.next().unwrap());
        if p.is_relative() {
            p = std::env::current_dir()?.join(p);
        }
        config.cache_dir = p;
    }
    if let Some(pyver) = matches.get_one::<String>("pyver") {
        if !pyver.is_empty()
            && pyver != "latest"
            && regex_find(r"^\d+\.\d+\.\d+$", &pyver).is_none()
        {
            return Err("版本号格式错误".into());
        }
        config.pyver = pyver.to_string();
    }
    config.is32 = matches.get_flag("32");
    config.skip_download = matches.get_flag("skip-download");
    if let Some(s) = matches.get_one::<String>("python-mirror") {
        config.python_mirror = s.to_string();
    }
    if let Some(s) = matches.get_one::<String>("pip-mirror") {
        config.pip_mirror = s.to_string();
    }
    config.keep_scripts = matches.get_flag("keep-scripts");
    config.keep_dist_info = matches.get_flag("keep-dist-info");
    config.keep_pip = matches.get_flag("keep-pip");
    config.packages = matches
        .get_many::<String>("PACKAGES")
        .unwrap_or_default()
        .map(|s| s.trim().to_string())
        .collect();
    if let Some(s) = matches.get_one::<String>("optimize") {
        config.optimize = s.parse()?;
    }

    let last_len = std::cell::Cell::new(0);
    let simple_progress = |total: i64, read: i64| {
        if total == -1 {
            if read == -1 {
                print!("\r{}", " ".repeat(last_len.get()));
                print!("\r");
            } else {
                let s = read.to_string();
                last_len.set(s.chars().count());
                print!("\r{}", s);
            }
        } else {
            let p = 100 * read / total;
            let p2 = 30 * read / total;
            let s = format!(
                "{}% [{}{}]",
                p,
                "#".repeat((p2) as _),
                "-".repeat((30 - p2) as _)
            );
            last_len.set(s.chars().count());
            print!("\r{}", s);
        }
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    };

    if atty::is(atty::Stream::Stdout) {
        run(&config, &simple_progress).await
    } else {
        run(&config, &|_: i64, _: i64| {}).await
    }
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
