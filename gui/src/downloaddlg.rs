use crate::{dialog, logger, resources, run, Config};
use winapi::shared::basetsd::INT_PTR;
use winapi::shared::minwindef::{LPARAM, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{IDCANCEL, NMHDR};

pub struct DownloadProc {
    config: std::sync::Arc<Config>,
    receiver: Option<std::sync::mpsc::Receiver<Msg>>,
}

impl DownloadProc {
    pub fn new(config: Config) -> Self {
        DownloadProc {
            config: std::sync::Arc::new(config),
            receiver: None,
        }
    }

    fn create_work_thread(&mut self) {
        let config = self.config.clone();
        let (s, r) = std::sync::mpsc::channel::<Msg>();
        self.receiver = Some(r);
        std::thread::spawn(move || {
            let mut rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(run(&config, &|a: i64, b: i64| {
                s.send(Msg::Progress(a, b)).unwrap();
            }));
            s.send(Msg::Result(result.map_err(|e| e.to_string())))
                .unwrap();
        });
    }
}

enum Msg {
    Progress(i64, i64),
    Result(Result<(), String>),
}

impl dialog::DialogProc for DownloadProc {
    fn on_init(&mut self, dlg: &dialog::Dialog) -> bool {
        let icon = dlg.load_icon(resources::ID_ICON_MAIN);
        dlg.set_icon(icon);
        logger::init(log::Level::Info).unwrap();
        logger::set_hwnd(
            dlg.get_item(resources::IDC_EDT_LOG),
            dlg.get_item(resources::IDC_STC_STATUS),
        );
        dlg.processbar_marquee(resources::IDC_PGB1, true);
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
                    dlg.processbar_marquee(resources::IDC_PGB1, true);
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
                        if total == -1 {
                            dlg.processbar_marquee(resources::IDC_PGB1, true);
                        } else {
                            if read == 0 {
                                dlg.processbar_marquee(resources::IDC_PGB1, false);
                                dlg.progressbar_set_range(resources::IDC_PGB1, 0, 100);
                                dlg.progressbar_set_pos(resources::IDC_PGB1, 0);
                            } else {
                                dlg.progressbar_set_pos(
                                    resources::IDC_PGB1,
                                    (100 * read / total) as _,
                                );
                            }
                        }
                    }
                    Msg::Result(r) => {
                        dlg.processbar_marquee(resources::IDC_PGB1, false);
                        dlg.progressbar_set_range(resources::IDC_PGB1, 0, 100);
                        dlg.progressbar_set_pos(resources::IDC_PGB1, 100);
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
        }
        false
    }

    fn on_message(
        &mut self,
        _dlg: &dialog::Dialog,
        _msg: UINT,
        _wp: WPARAM,
        _lp: LPARAM,
        _result: &mut INT_PTR,
    ) -> bool {
        false
    }
}
