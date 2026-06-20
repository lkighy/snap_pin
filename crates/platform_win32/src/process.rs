pub fn process_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: OpenProcess is called with query-only rights and a pid supplied by
    // our parent application. Handles are closed before returning.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }

    let mut exit_code = 0;
    // SAFETY: handle is a valid process handle from OpenProcess; exit_code is a
    // valid out pointer for the duration of the call.
    let ok = unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0;
    // SAFETY: handle was returned by OpenProcess and must be released once.
    let _ = unsafe { CloseHandle(handle) };

    ok && exit_code == STILL_ACTIVE as u32
}
