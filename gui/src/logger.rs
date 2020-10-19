#![allow(dead_code)]

use crate::to_wstring;

// 反正已经各种 unsafe 了，不差这一点。。
static mut LOGGER: EditBoxLogger = EditBoxLogger {
    hwnd: 0,
    hwnd_static: 0,
    limit: 0,
};

pub fn init(level: log::Level) -> Result<(), log::SetLoggerError> {
    unsafe {
        log::set_logger(&LOGGER)?;
    }
    log::set_max_level(level.to_level_filter());
    Ok(())
}

pub fn set_hwnd(hwnd: winapi::shared::windef::HWND, hwnd_static: winapi::shared::windef::HWND) {
    unsafe {
        LOGGER.hwnd = hwnd as _;
        LOGGER.hwnd_static = hwnd_static as _;
        // 设置一个更大的缓冲区
        winapi::um::winuser::SendMessageW(
            hwnd,
            winapi::um::winuser::EM_SETLIMITTEXT as _,
            1 * 1024 * 1024,
            0,
        );
        LOGGER.limit = winapi::um::winuser::SendMessageW(
            hwnd,
            winapi::um::winuser::EM_GETLIMITTEXT as _,
            0,
            0,
        ) as _;
    }
}

struct EditBoxLogger {
    hwnd: isize,
    hwnd_static: isize,
    limit: i32,
}

impl log::Log for EditBoxLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        unsafe {
            let hwnd = self.hwnd as winapi::shared::windef::HWND;
            if winapi::um::winuser::IsWindow(hwnd) == 0 {
                return;
            }
            // 通过 warn 级别日志来设置状态
            if record.level() == log::Level::Warn {
                let status = format!("{}", record.args());
                winapi::um::winuser::SetWindowTextW(
                    self.hwnd_static as _,
                    to_wstring(&status).as_ptr(),
                );
            }
            let msg = format!("{}\r\n", record.args());
            let mut len = winapi::um::winuser::GetWindowTextLengthW(hwnd);
            // msg.len() 虽然返回字节数而不是字符数，但这里宁多勿少
            if len + msg.len() as i32 > self.limit {
                winapi::um::winuser::SetWindowTextW(
                    hwnd,
                    to_wstring("==== 在此截断 ====\r\n").as_ptr(),
                );
                len = winapi::um::winuser::GetWindowTextLengthW(hwnd);
            }
            winapi::um::winuser::SendMessageW(
                hwnd,
                winapi::um::winuser::EM_SETSEL as _,
                len as _,
                len as _,
            );
            winapi::um::winuser::SendMessageW(
                hwnd,
                winapi::um::winuser::EM_REPLACESEL as _,
                0,
                to_wstring(&msg).as_ptr() as _,
            );
            // 滚动到底部
            let len = winapi::um::winuser::GetWindowTextLengthW(hwnd);
            winapi::um::winuser::SendMessageW(
                hwnd,
                winapi::um::winuser::EM_SETSEL as _,
                len as _,
                len as _,
            );
            winapi::um::winuser::SendMessageW(hwnd, winapi::um::winuser::EM_SCROLLCARET as _, 0, 0);
        }
    }

    fn flush(&self) {}
}
