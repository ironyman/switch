use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
    Win32::System::SystemServices::*,
    Win32::Security::Authorization::*,
    Win32::Security::*,
    Win32::System::Memory::*,
};

// use crate::log::*;

pub unsafe fn create_process(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let created = CreateProcessW(
        PCWSTR(std::ptr::null()),
        PWSTR(cmdline.as_mut_ptr() as *mut _),
        std::ptr::null(),
        std::ptr::null(),
        BOOL(0),
        PROCESS_CREATION_FLAGS(0),
        std::ptr::null(),
        PCWSTR(std::ptr::null()),
        &si,
        &mut pi
    );
    if !created.as_bool() {
        return Err(Error::from_win32());
    }

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    return Ok(pi.dwProcessId);
}

pub unsafe fn create_medium_process(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut token = HANDLE(0);
    if !OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_DUPLICATE | TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
        &mut token).as_bool()
    {
        return Err(Error::from_win32());
    }

    let mut new_token = HANDLE(0);
    if !DuplicateTokenEx(token,
        TOKEN_ACCESS_MASK(0),
        std::ptr::null(),
        SecurityImpersonation,
        TokenPrimary,
        &mut new_token).as_bool()
    {
        CloseHandle(token);
        return Err(Error::from_win32());
    }

    let mut sid =  PSID::default();
    let medium_integrity = "S-1-16-8192\0".encode_utf16().collect::<Vec<u16>>();
    if !ConvertStringSidToSidW(PCWSTR(medium_integrity.as_ptr()), &mut sid).as_bool() {
        CloseHandle(token);
        CloseHandle(new_token);
        return Err(Error::from_win32());
    }

    let mut til = TOKEN_MANDATORY_LABEL::default();
    til.Label.Attributes = SE_GROUP_INTEGRITY as u32;
    til.Label.Sid = sid;

    if !SetTokenInformation(new_token,
        TokenIntegrityLevel,
        std::mem::transmute(&til),
        std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid)).as_bool()
    {
        LocalFree(std::mem::transmute(sid));
        CloseHandle(token);
        CloseHandle(new_token);
        return Err(Error::from_win32());
    }

    SetLastError(NO_ERROR);
    let created = CreateProcessAsUserW(
        new_token,
        PCWSTR(std::ptr::null()),
        PWSTR(cmdline.as_mut_ptr() as *mut _),
        std::ptr::null(),
        std::ptr::null(),
        BOOL(0),
        0,
        std::ptr::null(),
        PCWSTR(std::ptr::null()),
        &si,
        &mut pi
    );

    LocalFree(std::mem::transmute(sid));

    CloseHandle(token);
    CloseHandle(new_token);
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    if !created.as_bool() {
        return Err(Error::from_win32());
    }

    return Ok(pi.dwProcessId);
}

pub fn get_installed_exe_path(file: &str) -> String {
    let mut install_path  = std::path::PathBuf::from(std::env::current_exe().unwrap().parent().unwrap());
    install_path.push(file);
    return install_path.into_os_string().into_string().unwrap();
}
