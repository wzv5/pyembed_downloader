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
        .version(clap::crate_version!())
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
            clap::Arg::with_name("python-mirror")
                .long("python-mirror")
                .takes_value(true)
                .value_name("url")
                .help("通过指定的镜像站下载 Python 安装包"),
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
            clap::Arg::with_name("optimize")
                .long("optimize")
                .takes_value(true)
                .value_name("level")
                .default_value("0")
                .help(
                    "优化编译级别：0（不优化），1（删除断言，关闭调试），2（同时删除文档字符串）",
                ),
        )
        .arg(
            clap::Arg::with_name("PACKAGES")
                .index(1)
                .multiple(true)
                .help("要安装的 pip 依赖包"),
        )
        .get_matches();

    let dir = match matches.value_of_os("dir") {
        None => std::env::current_dir()?,
        Some(s) => std::path::PathBuf::from(s),
    };
    let pyver = matches.value_of("pyver").unwrap_or_default().to_string();
    let is32 = matches.is_present("32");
    let skip_download = matches.is_present("skip-download");
    let python_mirror = matches
        .value_of("python-mirror")
        .unwrap_or_default()
        .to_string();
    let pip_mirror = matches
        .value_of("pip-mirror")
        .unwrap_or_default()
        .to_string();
    let keep_scripts = matches.is_present("keep-scripts");
    let keep_dist_info = matches.is_present("keep-dist-info");
    let keep_pip = matches.is_present("keep-pip");
    let packages: Vec<&str> = matches.values_of("PACKAGES").unwrap_or_default().collect();
    let optimize = matches
        .value_of("optimize")
        .unwrap_or_default()
        .parse()
        .unwrap();

    if !pyver.is_empty() && pyver != "latest" && regex_find(r"^\d+\.\d+\.\d+$", &pyver).is_none() {
        return Err("版本号格式错误".into());
    }

    let config = Config {
        dir,
        pyver,
        is32,
        skip_download,
        python_mirror,
        pip_mirror,
        keep_scripts,
        keep_dist_info,
        keep_pip,
        optimize,
        packages: packages.iter().map(|s| s.to_string()).collect(),
    };

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
