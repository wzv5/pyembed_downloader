use crate::Result;

// https://github.com/rust-lang/cargo/blob/master/src/cargo/util/job.rs
// 简单一抄，凑合能用
pub(crate) fn setup_job() -> Result<()> {
    unsafe {
        use winapi::shared::minwindef::*;
        use winapi::um::jobapi2::*;
        use winapi::um::processthreadsapi::*;
        use winapi::um::winnt::*;

        let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
        if job.is_null() {
            return Err("CreateJobObject failed".into());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &mut info as *mut _ as LPVOID,
            std::mem::size_of_val(&info) as DWORD,
        );
        if r == 0 {
            return Err("SetInformationJobObject failed".into());
        }
        let me = GetCurrentProcess();
        let r = AssignProcessToJobObject(job, me);
        if r == 0 {
            return Err("AssignProcessToJobObject failed".into());
        }
        Ok(())
    }
}

pub fn regex_find<'a>(re: &str, text: &'a str) -> Option<regex::Captures<'a>> {
    if let Ok(re) = regex::RegexBuilder::new(re)
        .dot_matches_new_line(true)
        .build()
    {
        return re.captures(text);
    }
    None
}
