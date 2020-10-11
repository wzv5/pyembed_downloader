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

    let workdir = match matches.value_of_os("dir") {
        None => std::env::current_dir()?,
        Some(s) => std::path::PathBuf::from(s),
    };
    let pyver = matches.value_of("pyver").unwrap_or_default().to_string();
    let is32 = matches.is_present("32");
    let skipdownload = matches.is_present("skip-download");
    let pipmirror = matches
        .value_of("pip-mirror")
        .unwrap_or_default()
        .to_string();
    let keepscripts = matches.is_present("keep-scripts");
    let keepdistinfo = matches.is_present("keep-dist-info");
    let keeppip = matches.is_present("keep-pip");
    let packages: Vec<&str> = matches.values_of("PACKAGES").unwrap_or_default().collect();

    if !pyver.is_empty() && pyver != "latest" && regex_find(r"^\d+\.\d+\.\d+$", &pyver).is_none() {
        return Err("版本号格式错误".into());
    }

    let config = Config {
        dir: workdir,
        pyver: pyver,
        is32: is32,
        skip_download: skipdownload,
        pip_mirror: pipmirror,
        keep_scripts: keepscripts,
        keep_dist_info: keepdistinfo,
        keep_pip: keeppip,
        packages: packages.iter().map(|s| s.to_string()).collect(),
    };

    let simple_progress = |total: i64, read: i64| {
        if total == -1 {
            if read == -1 {
                print!("\r");
            } else {
                print!("\r{}", read);
            }
        } else {
            print!("\r{}%", 100 * read / total);
        }
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    };

    run(&config, &simple_progress).await
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
