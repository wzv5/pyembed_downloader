use crate::Result;

// https://github.com/rust-lang/cargo/blob/master/src/cargo/util/job.rs
// 简单一抄，凑合能用
pub(crate) fn setup_job() -> Result<Job> {
    unsafe {
        use winapi::shared::minwindef::*;
        use winapi::um::jobapi2::*;
        use winapi::um::processthreadsapi::*;
        use winapi::um::winnt::*;

        let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
        if job.is_null() {
            return Err("CreateJobObject failed".into());
        }
        let job = Handle { inner: job };
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = SetInformationJobObject(
            job.inner,
            JobObjectExtendedLimitInformation,
            &mut info as *mut _ as LPVOID,
            std::mem::size_of_val(&info) as DWORD,
        );
        if r == 0 {
            return Err("SetInformationJobObject failed".into());
        }
        let me = GetCurrentProcess();
        let r = AssignProcessToJobObject(job.inner, me);
        if r == 0 {
            return Err("AssignProcessToJobObject failed".into());
        }
        Ok(Job { handle: job })
    }
}

pub struct Job {
    handle: Handle,
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let mut info: winapi::um::winnt::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
            info = std::mem::zeroed();
            let r = winapi::um::jobapi2::SetInformationJobObject(
                self.handle.inner,
                winapi::um::winnt::JobObjectExtendedLimitInformation,
                &mut info as *mut _ as _,
                std::mem::size_of_val(&info) as _,
            );
            if r == 0 {
                info!(
                    "failed to configure job object to defaults: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
    }
}

pub struct Handle {
    inner: winapi::um::winnt::HANDLE,
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            winapi::um::handleapi::CloseHandle(self.inner);
        }
    }
}

pub(crate) fn regex_find<'a>(re: &str, text: &'a str) -> Option<regex::Captures<'a>> {
    if let Ok(re) = regex::RegexBuilder::new(re)
        .dot_matches_new_line(true)
        .build()
    {
        return re.captures(text);
    }
    None
}

pub(crate) fn md5(data: &[u8]) -> String {
    use md5::Digest;
    let mut buf = [0u8; 32];
    let hash = md5::Md5::digest(data);
    base16ct::lower::encode_str(&hash, &mut buf)
        .unwrap()
        .to_string()
}

pub(crate) fn sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let mut buf = [0u8; 64];
    let hash = sha2::Sha256::digest(data);
    base16ct::lower::encode_str(&hash, &mut buf)
        .unwrap()
        .to_string()
}
