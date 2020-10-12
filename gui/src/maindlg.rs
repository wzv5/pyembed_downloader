use crate::{dialog, resources, Config, to_wstring};
use winapi::shared::basetsd::INT_PTR;
use winapi::shared::minwindef::{LPARAM, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::commctrl::{NM_CLICK, NM_RETURN, PNMLINK};
use winapi::um::winuser;
use winapi::um::winuser::{IDCANCEL, NMHDR, SW_SHOW};

pub struct MainProc<'a> {
    pub config: &'a mut Config,
}

impl<'a> dialog::DialogProc for MainProc<'a> {
    fn on_init(&mut self, dlg: &dialog::Dialog) -> bool {
        if let Some(v) = option_env!("CARGO_PKG_VERSION") {
            dlg.set_title(&format!("{} v{}", dlg.get_title(), v));
        }
        // icon 没有释放，随便了
        let icon = dlg.load_icon(resources::ID_ICON_MAIN);
        dlg.set_icon(icon);
        dlg.send_item_message(
            resources::IDC_BTN_START,
            winapi::um::commctrl::BCM_SETNOTE,
            0,
            to_wstring("还等什么？").as_ptr() as _,
        );
        // 设置控件初始值
        dlg.set_item_text(resources::IDC_EDT_DIR, self.config.dir.to_str().unwrap());
        dlg.set_item_text(resources::IDC_EDT_VER, &self.config.pyver);
        dlg.set_enable(resources::IDC_EDT_VER, self.config.pyver != "latest");
        dlg.set_check(resources::IDC_CHK_VER, self.config.pyver != "latest");
        dlg.set_check(resources::IDC_CHK_32, self.config.is32);
        dlg.set_check(resources::IDC_CHK_SKIP_DOWNLOAD, self.config.skip_download);
        dlg.set_check(resources::IDC_CHK_KEEP_SCRIPTS, self.config.keep_scripts);
        dlg.set_check(
            resources::IDC_CHK_KEEP_DIST_INFO,
            self.config.keep_dist_info,
        );
        dlg.set_check(resources::IDC_CHK_KEEP_PIP, self.config.keep_pip);
        for i in vec!["", "https://pypi.tuna.tsinghua.edu.cn/simple"] {
            dlg.send_item_message(
                resources::IDC_CBO_MIRROR,
                winuser::CB_ADDSTRING,
                0,
                to_wstring(i).as_ptr() as _,
            );
        }
        dlg.set_item_text(resources::IDC_CBO_MIRROR, &self.config.pip_mirror);
        let mut packages = "".to_string();
        for i in self.config.packages.iter() {
            packages += &format!("{}\r\n", i);
        }
        dlg.set_item_text(resources::IDC_EDT_PACKAGES, &packages);
        // 添加 tooltip
        dlg.set_tooltip(resources::IDC_CHK_VER, "下载指定版本的 Python，如 3.8.6，默认下载最新版");
        dlg.set_tooltip(resources::IDC_CHK_32, "下载 32 位版本，默认下载 64 位版本");
        dlg.set_tooltip(resources::IDC_CHK_SKIP_DOWNLOAD, "跳过下载，用于下载后想要添加或更新依赖包");
        dlg.set_tooltip(resources::IDC_CHK_KEEP_SCRIPTS, "保留 Scripts 目录");
        dlg.set_tooltip(resources::IDC_CHK_KEEP_DIST_INFO, "保留 dist-info 目录，删除此目录后将无法再通过 pip 管理依赖");
        dlg.set_tooltip(resources::IDC_CHK_KEEP_PIP, "保留 pip、setuptools、wheel 依赖包");
        // 设置初始焦点
        unsafe {
            winuser::SetFocus(dlg.get_item(resources::IDC_EDT_PACKAGES));
        }
        // 返回 false 避免系统自动设置焦点
        false
    }

    fn on_command(&mut self, dlg: &dialog::Dialog, id: i32, _code: i32, _hwnd: HWND) -> bool {
        match id {
            IDCANCEL => {
                dlg.end_dialog(0);
                true
            }
            resources::IDC_BTN_START => {
                // 检查用户输入
                let packages: Vec<String> = dlg
                    .get_item_text(resources::IDC_EDT_PACKAGES)
                    .split_whitespace()
                    .map(|i| i.to_string())
                    .collect();
                if packages.len() == 0 {
                    if dlg.message_box(
                        "没有指定要安装的依赖包，确定要继续吗？",
                        crate::APP_NAME,
                        winuser::MB_ICONQUESTION | winuser::MB_OKCANCEL,
                    ) != winuser::IDOK
                    {
                        return true;
                    }
                }
                let mut ver = if dlg.get_check(resources::IDC_CHK_VER) {
                    dlg.get_item_text(resources::IDC_EDT_VER)
                } else {
                    "latest".into()
                };
                if ver.is_empty() {
                    ver = "latest".into();
                }
                if ver != "latest" {
                    if !ver.is_empty() && ver != "latest" && regex_find(r"^\d+\.\d+\.\d+$", &ver).is_none() {
                        dlg.message_box("版本号格式错误", crate::APP_NAME, winapi::um::winuser::MB_ICONERROR);
                        return true;
                    }
                }
                // 修改配置
                self.config.dir = dlg.get_item_text(resources::IDC_EDT_DIR).into();
                self.config.pyver = ver;
                self.config.is32 = dlg.get_check(resources::IDC_CHK_32);
                self.config.skip_download = dlg.get_check(resources::IDC_CHK_SKIP_DOWNLOAD);
                self.config.pip_mirror = dlg.get_item_text(resources::IDC_CBO_MIRROR);
                self.config.keep_scripts = dlg.get_check(resources::IDC_CHK_KEEP_SCRIPTS);
                self.config.keep_dist_info = dlg.get_check(resources::IDC_CHK_KEEP_DIST_INFO);
                self.config.keep_pip = dlg.get_check(resources::IDC_CHK_KEEP_PIP);
                self.config.packages = packages;
                // 返回 1 表示检查没问题，开始下载
                dlg.end_dialog(1);
                true
            }
            resources::IDC_CHK_VER => {
                dlg.set_enable(
                    resources::IDC_EDT_VER,
                    dlg.get_check(resources::IDC_CHK_VER),
                );
                true
            }
            resources::IDC_BTN_DIR => {
                let params = wfd::DialogParams {
                    options: wfd::FOS_PICKFOLDERS,
                    title: "Select a directory",
                    ..Default::default()
                };
                if let Ok(result) = wfd::open_dialog(params) {
                    dlg.set_item_text(
                        resources::IDC_EDT_DIR,
                        result.selected_file_path.to_str().unwrap(),
                    );
                }
                true
            }
            _ => false,
        }
    }

    fn on_notify(&mut self, dlg: &dialog::Dialog, _id: i32, nmhdr: &NMHDR) -> bool {
        if nmhdr.idFrom as i32 == resources::IDC_LNK1 {
            if nmhdr.code == NM_CLICK || nmhdr.code == NM_RETURN {
                let nmlink = unsafe { (nmhdr as *const _ as PNMLINK).as_ref().unwrap() };
                unsafe {
                    winapi::um::shellapi::ShellExecuteW(
                        dlg.get_hwnd(),
                        to_wstring("open").as_ptr(),
                        nmlink.item.szUrl.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        SW_SHOW,
                    );
                }
                return true;
            }
        }
        false
    }

    fn on_timer(&mut self, _dlg: &dialog::Dialog, _id: i32) -> bool {
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

fn regex_find<'a>(re: &str, text: &'a str) -> Option<regex::Captures<'a>> {
    if let Ok(re) = regex::RegexBuilder::new(re)
        .dot_matches_new_line(true)
        .build()
    {
        return re.captures(text);
    }
    None
}
