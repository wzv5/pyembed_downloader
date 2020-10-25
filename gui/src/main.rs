#![windows_subsystem = "windows"]

#[macro_use]
extern crate log;

mod dialog;
mod downloaddlg;
mod logger;
mod maindlg;
mod resources;

use pyembed_downloader::{run, Config};

static APP_NAME: &'static str = "pyembed_downloader";

fn main() {
    unsafe {
        winapi::um::objbase::CoInitialize(std::ptr::null_mut());
    }

    let mut config = Config::default();
    if dialog::Dialog::show(
        resources::IDD_DLG_MAIN,
        Some(&mut maindlg::MainProc {
            config: &mut config,
        }),
    ) == 1
    {
        dialog::Dialog::show(
            resources::IDD_DLG_DOWNLOAD,
            Some(&mut downloaddlg::DownloadProc::new(config)),
        );
    }
}

fn to_wstring(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}
