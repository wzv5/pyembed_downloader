use crate::{dialog, logger, resources, run, Config};
use winapi::shared::minwindef::LRESULT;
use winapi::shared::minwindef::{LPARAM, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{IDCANCEL, NMHDR};

pub struct DownloadProc<'a> {
    config: std::sync::Arc<Config>,
    receiver: Option<std::sync::mpsc::Receiver<Msg>>,
    taskbar: &'a winapi::um::shobjidl_core::ITaskbarList3,
}

impl<'a> DownloadProc<'a> {
    pub fn new(config: Config) -> Self {
        let taskbar = unsafe {
            let mut taskbar: winapi::shared::minwindef::LPVOID = std::ptr::null_mut();
            winapi::um::combaseapi::CoCreateInstance(
                &winapi::um::shobjidl_core::CLSID_TaskbarList,
                std::ptr::null_mut(),
                winapi::um::combaseapi::CLSCTX_ALL,
                &<winapi::um::shobjidl_core::ITaskbarList3 as winapi::Interface>::uuidof(),
                &mut taskbar,
            );
            &*(taskbar as *mut winapi::um::shobjidl_core::ITaskbarList3)
        };

        DownloadProc {
            config: std::sync::Arc::new(config),
            receiver: None,
            taskbar,
        }
    }

    fn create_work_thread(&mut self) {
        let config = self.config.clone();
        let (s, r) = std::sync::mpsc::channel::<Msg>();
        self.receiver = Some(r);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().build().unwrap();
            let result = rt.block_on(run(&config, &|a: i64, b: i64| {
                s.send(Msg::Progress(a, b)).unwrap();
            }));
            s.send(Msg::Result(result.map_err(|e| e.to_string())))
                .unwrap();
        });
    }

    fn set_progress(&self, dlg: &dialog::Dialog, total: i64, current: i64) {
        if total == -1 {
            if current == -1 {
                dlg.processbar_marquee(resources::IDC_PGB1, true);
                unsafe {
                    self.taskbar.SetProgressState(
                        dlg.get_hwnd(),
                        winapi::um::shobjidl_core::TBPF_NOPROGRESS,
                    );
                }
            }
        } else {
            if current == 0 {
                dlg.processbar_marquee(resources::IDC_PGB1, false);
                dlg.progressbar_set_range(resources::IDC_PGB1, 0, 100);
                unsafe {
                    self.taskbar
                        .SetProgressState(dlg.get_hwnd(), winapi::um::shobjidl_core::TBPF_NORMAL);
                }
            }
            dlg.progressbar_set_pos(resources::IDC_PGB1, (100 * current / total) as _);
            unsafe {
                self.taskbar
                    .SetProgressValue(dlg.get_hwnd(), current as _, total as _);
            }
        }
    }
}

impl<'a> Drop for DownloadProc<'a> {
    fn drop(&mut self) {
        unsafe {
            self.taskbar.Release();
        }
    }
}

enum Msg {
    Progress(i64, i64),
    Result(Result<(), String>),
}

impl<'a> dialog::DialogProc for DownloadProc<'a> {
    fn on_init(&mut self, dlg: &dialog::Dialog) -> bool {
        let icon = dlg.load_icon(resources::ID_ICON_MAIN);
        dlg.set_icon(icon);
        logger::init(log::Level::Info).unwrap();
        logger::set_hwnd(
            dlg.get_item(resources::IDC_EDT_LOG),
            dlg.get_item(resources::IDC_STC_STATUS),
        );
        self.set_progress(dlg, -1, -1);
        self.create_work_thread();
        dlg.set_timer(resources::ID_TIMER_RECEIVER, 100);
        false
    }

    fn on_command(&mut self, dlg: &dialog::Dialog, id: i32, _code: i32, _hwnd: HWND) -> bool {
        match id {
            IDCANCEL => {
                dlg.end_dialog(0);
                true
            }
            resources::IDC_BTN_EXIT => {
                let text = dlg.get_item_text(id);
                if text == "重试" {
                    dlg.progressbar_set_state(
                        resources::IDC_PGB1,
                        winapi::um::commctrl::PBST_NORMAL,
                    );
                    dlg.set_item_text(id, "取消");
                    self.set_progress(dlg, -1, -1);
                    self.create_work_thread();
                    dlg.set_timer(resources::ID_TIMER_RECEIVER, 100);
                } else {
                    dlg.end_dialog(0);
                }
                true
            }
            _ => false,
        }
    }

    fn on_notify(&mut self, _dlg: &dialog::Dialog, _id: i32, _nmhdr: &NMHDR) -> bool {
        false
    }

    fn on_timer(&mut self, dlg: &dialog::Dialog, id: i32) -> bool {
        if id == resources::ID_TIMER_RECEIVER {
            while let Ok(msg) = self.receiver.as_ref().unwrap().try_recv() {
                match msg {
                    Msg::Progress(total, read) => {
                        self.set_progress(dlg, total, read);
                    }
                    Msg::Result(r) => {
                        self.set_progress(dlg, 100, 0);
                        self.set_progress(dlg, 100, 100);
                        match r {
                            Ok(_) => {
                                dlg.set_item_text(resources::IDC_BTN_EXIT, "完成");
                                dlg.message_box(
                                    "完成！",
                                    crate::APP_NAME,
                                    winapi::um::winuser::MB_ICONINFORMATION,
                                );
                            }
                            Err(e) => {
                                dlg.progressbar_set_state(
                                    resources::IDC_PGB1,
                                    winapi::um::commctrl::PBST_ERROR,
                                );
                                unsafe {
                                    self.taskbar.SetProgressState(
                                        dlg.get_hwnd(),
                                        winapi::um::shobjidl_core::TBPF_ERROR,
                                    );
                                }
                                dlg.set_item_text(resources::IDC_BTN_EXIT, "重试");
                                let s = format!("错误：{}", e);
                                error!("{}", s);
                                dlg.message_box(
                                    &s,
                                    crate::APP_NAME,
                                    winapi::um::winuser::MB_ICONERROR,
                                );
                            }
                        };
                        dlg.kill_timer(id);
                    }
                };
            }
            return true;
        }
        false
    }

    fn on_message(
        &mut self,
        _dlg: &dialog::Dialog,
        _msg: UINT,
        _wp: WPARAM,
        _lp: LPARAM,
        _result: &mut LRESULT,
    ) -> bool {
        false
    }
}
