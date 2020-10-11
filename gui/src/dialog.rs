#![allow(dead_code)]

use crate::to_wstring;
use winapi::shared::basetsd::{INT_PTR, UINT_PTR};
use winapi::shared::minwindef::{DWORD, HINSTANCE, HIWORD, LOWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HICON, HWND};
use winapi::um::commctrl::{
    PBM_GETPOS, PBM_SETMARQUEE, PBM_SETPOS, PBM_SETRANGE, PBS_MARQUEE, TOOLINFOW, TOOLTIPS_CLASS,
    TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTS_ALWAYSTIP,
};
use winapi::um::winuser;
use winapi::um::winuser::{
    CW_USEDEFAULT, GWLP_USERDATA, GWL_STYLE, ICON_BIG, ICON_SMALL, LPNMHDR, MAKEINTRESOURCEW,
    NMHDR, WM_COMMAND, WM_INITDIALOG, WM_NOTIFY, WM_SETICON, WM_TIMER, WS_POPUP,
};

// 对话框消息处理
// 如果返回 true 则表示已处理该消息，并使用 result 作为返回值
// 先调用 on_message，如果 on_message 返回 true 就不再调用其他处理方法
#[rustfmt::skip]
pub trait DialogProc {
    fn on_init(&mut self, dlg: &Dialog) -> bool;
    fn on_command(&mut self, dlg: &Dialog, id: i32, code: i32, hwnd: HWND) -> bool;
    fn on_notify(&mut self, dlg: &Dialog, id: i32, nmhdr: &NMHDR) -> bool;
    fn on_timer(&mut self, dlg: &Dialog, id: i32) -> bool;
    fn on_message(&mut self, dlg: &Dialog, msg: UINT, wp: WPARAM, lp: LPARAM, result: &mut INT_PTR) -> bool;
}

pub struct Dialog<'a> {
    proc: Option<&'a mut dyn DialogProc>,
    instance: HINSTANCE,
    hwnd: HWND,
}

impl<'a> Dialog<'a> {
    pub fn show(id: u16, proc: Option<&'a mut dyn DialogProc>) -> INT_PTR {
        unsafe {
            let mut dlg = Dialog {
                proc,
                instance: winapi::um::libloaderapi::GetModuleHandleW(std::ptr::null_mut()),
                hwnd: std::ptr::null_mut(),
            };
            winuser::DialogBoxParamW(
                dlg.instance,
                MAKEINTRESOURCEW(id),
                std::ptr::null_mut(),
                Some(Self::dlgproc),
                &mut dlg as *mut _ as _,
            )
        }
    }

    pub fn get_instance(&self) -> HINSTANCE {
        self.instance
    }

    pub fn get_hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn get_item(&self, id: i32) -> HWND {
        let hwnd = unsafe { winuser::GetDlgItem(self.hwnd, id) };
        if hwnd.is_null() {
            panic!("找不到对象");
        }
        hwnd
    }

    pub fn load_icon(&self, id: u16) -> HICON {
        unsafe { winuser::LoadIconW(self.instance, MAKEINTRESOURCEW(id)) }
    }

    pub fn set_icon(&self, icon: HICON) {
        self.send_message(WM_SETICON, ICON_SMALL as _, icon as _);
        self.send_message(WM_SETICON, ICON_BIG as _, icon as _);
    }

    pub fn end_dialog(&self, result: INT_PTR) -> bool {
        unsafe { winuser::EndDialog(self.hwnd, result) != 0 }
    }

    pub fn get_item_text(&self, id: i32) -> String {
        unsafe {
            let len = winapi::um::winuser::GetWindowTextLengthW(self.get_item(id)) + 1;
            let mut buf = vec![0; len as _];
            let ret = winuser::GetDlgItemTextW(self.hwnd, id, buf.as_mut_ptr(), buf.len() as _);
            String::from_utf16(&buf[..ret as _]).unwrap()
        }
    }

    pub fn set_item_text(&self, id: i32, text: &str) {
        unsafe {
            winuser::SetDlgItemTextW(self.hwnd, id as _, to_wstring(text).as_ptr());
        }
    }

    pub fn set_enable(&self, id: i32, enable: bool) {
        unsafe {
            winuser::EnableWindow(self.get_item(id), enable as _);
        }
    }

    pub fn get_enable(&self, id: i32) -> bool {
        unsafe { winuser::IsWindowEnabled(self.get_item(id)) != 0 }
    }

    pub fn set_check(&self, id: i32, check: bool) {
        unsafe {
            winuser::CheckDlgButton(self.hwnd, id, check as _);
        }
    }

    pub fn get_check(&self, id: i32) -> bool {
        unsafe { winuser::IsDlgButtonChecked(self.hwnd, id) != 0 }
    }

    pub fn set_tooltip(&self, id: i32, tooltip: &str) {
        unsafe {
            let hwnd_tool = self.get_item(id);
            let hwnd_tip = winuser::CreateWindowExW(
                0,
                to_wstring(TOOLTIPS_CLASS).as_ptr(),
                std::ptr::null(),
                WS_POPUP | TTS_ALWAYSTIP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                self.hwnd,
                std::ptr::null_mut(),
                self.instance,
                std::ptr::null_mut(),
            );
            if !hwnd_tip.is_null() {
                let mut info = TOOLINFOW::default();
                info.cbSize = std::mem::size_of::<TOOLINFOW>() as _;
                info.hwnd = self.hwnd;
                info.uId = hwnd_tool as _;
                info.lpszText = to_wstring(tooltip).as_mut_ptr();
                info.uFlags = TTF_IDISHWND | TTF_SUBCLASS;
                self.send_message(TTM_ADDTOOLW, 0, &info as *const _ as _);
            }
        }
    }

    pub fn send_message(&self, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { winuser::SendMessageW(self.hwnd, msg, wp, lp) }
    }

    pub fn send_item_message(&self, id: i32, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { winuser::SendDlgItemMessageW(self.hwnd, id, msg, wp, lp) }
    }

    pub fn set_timer(&self, id: i32, elapse: i32) -> UINT_PTR {
        unsafe { winuser::SetTimer(self.hwnd, id as _, elapse as _, None) }
    }

    pub fn kill_timer(&self, id: i32) -> bool {
        unsafe { winuser::KillTimer(self.hwnd, id as _) != 0 }
    }

    pub fn processbar_marquee(&self, id: i32, enable: bool) {
        unsafe {
            let hwnd = self.get_item(id);
            let mut style = winuser::GetWindowLongW(hwnd, GWL_STYLE) as DWORD;
            if enable {
                style |= PBS_MARQUEE;
            } else {
                style &= !PBS_MARQUEE;
            }
            winuser::SetWindowLongW(hwnd, GWL_STYLE, style as _);
            if enable {
                winuser::SendMessageW(hwnd, PBM_SETMARQUEE, enable as _, 0);
            }
        }
    }

    pub fn processbar_is_marquee(&self, id: i32) -> bool {
        unsafe {
            let hwnd = self.get_item(id);
            let style = winuser::GetWindowLongW(hwnd, GWL_STYLE) as DWORD;
            (style & PBS_MARQUEE) != 0
        }
    }

    pub fn progressbar_set_range(&self, id: i32, min: u16, max: u16) {
        let range = winapi::shared::minwindef::MAKELONG(min, max);
        self.send_item_message(id, PBM_SETRANGE, 0, range as _);
    }

    pub fn progressbar_set_pos(&self, id: i32, pos: i32) {
        self.send_item_message(id, PBM_SETPOS, pos as _, 0);
    }

    pub fn progressbar_get_pos(&self, id: i32) -> i32 {
        self.send_item_message(id, PBM_GETPOS, 0, 0) as _
    }

    pub fn message_box(&self, text: &str, caption: &str, typ: u32) -> i32 {
        unsafe {
            winuser::MessageBoxW(
                self.hwnd,
                to_wstring(text).as_ptr(),
                to_wstring(caption).as_ptr(),
                typ,
            )
        }
    }

    unsafe fn get_self(hwnd: HWND) -> *mut Self {
        winuser::GetWindowLongPtrW(hwnd, GWLP_USERDATA) as _
    }

    unsafe extern "system" fn dlgproc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> INT_PTR {
        let this: *mut Self;
        if msg == WM_INITDIALOG {
            winuser::SetWindowLongPtrW(hwnd, GWLP_USERDATA, lp);
            this = lp as _;
            this.as_mut().unwrap().hwnd = hwnd;
        } else {
            this = Self::get_self(hwnd);
        }
        if !this.is_null() {
            let ref mut proc = this.as_mut().unwrap().proc;
            if let Some(proc) = proc {
                let dlg = this.as_ref().unwrap();
                let mut result: INT_PTR = 0;
                if proc.on_message(dlg, msg, wp, lp, &mut result) {
                    return result;
                }
                return match msg {
                    WM_INITDIALOG => proc.on_init(dlg),
                    WM_COMMAND => {
                        let id = LOWORD(wp as _) as i32;
                        let code = HIWORD(wp as _) as i32;
                        proc.on_command(dlg, id, code, lp as _)
                    }
                    WM_NOTIFY => {
                        let nmhdr = (lp as LPNMHDR).as_ref().unwrap();
                        proc.on_notify(dlg, wp as _, nmhdr)
                    }
                    WM_TIMER => proc.on_timer(dlg, wp as _),
                    _ => false,
                } as _;
            }
        }
        0
    }
}
