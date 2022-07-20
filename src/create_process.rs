use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
    Win32::System::SystemServices::*,
    Win32::Security::Authorization::*,
    Win32::Security::*,
    Win32::Security::AppLocker::*,
    Win32::System::Memory::*,
};

use crate::log::*;

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

unsafe fn set_privilege(token: HANDLE, privilege: String, enable: bool) -> bool {
    let mut luid = LUID::default();

    if !LookupPrivilegeValueA(
            PCSTR(std::ptr::null()),
            PCSTR((privilege + "\0").as_ptr()),
            &mut luid).as_bool()
    {
        return false;
    }

    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [
            LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: if enable { SE_PRIVILEGE_ENABLED } else { TOKEN_PRIVILEGES_ATTRIBUTES(0) }
            }
        ]
    };

    if !AdjustTokenPrivileges(
           token, 
           BOOL(0), 
           &tp, 
           std::mem::size_of::<TOKEN_PRIVILEGES>().try_into().unwrap(), 
           std::ptr::null_mut(),
           std::ptr::null_mut()
        ).as_bool()
    { 
        return false;
    }

    if GetLastError() == ERROR_NOT_ALL_ASSIGNED {
        return false;
    } 

    return true;
}



// Using the shell token.
pub unsafe fn create_medium_process(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut self_token = HANDLE(0);
    if !OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ADJUST_GROUPS | TOKEN_ADJUST_PRIVILEGES,
        &mut self_token).as_bool()
    {
        return Err(Error::from_win32());
    }

    if !set_privilege(self_token, "SeIncreaseQuotaPrivilege".into(), true) {
        crate::trace!("start", log::Level::Info, "Start app: set_privilege {:?}", Error::from_win32());
        return Err(Error::from_win32());
    }
    CloseHandle(self_token);

    let shell = windows::Win32::UI::WindowsAndMessaging::GetShellWindow();
    
    let mut shell_pid: u32 = 0;
    windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(shell, &mut shell_pid);

    let processh = OpenProcess(PROCESS_QUERY_INFORMATION, BOOL(0), shell_pid);

    let mut token = HANDLE(0);
    if !OpenProcessToken(
        processh,
        TOKEN_DUPLICATE | TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
        &mut token).as_bool()
    {
        CloseHandle(processh);
        return Err(Error::from_win32());
    }
    CloseHandle(processh);

    let mut new_token = HANDLE(0);
    if !DuplicateTokenEx(token,
        TOKEN_ACCESS_MASK(395),
        // TOKEN_ALL_ACCESS,
        std::ptr::null(),
        SecurityImpersonation,
        TokenPrimary,
        &mut new_token).as_bool()
    {
        CloseHandle(token);
        return Err(Error::from_win32());
    }

    if !ImpersonateLoggedOnUser(new_token).as_bool() {
        crate::trace!("start", log::Level::Info, "Start app: ImpersonateLoggedOnUser {:?} {:?}", Error::from_win32(), new_token);
        CloseHandle(token);
        CloseHandle(new_token);
        return Err(Error::from_win32());
    }

    SetLastError(NO_ERROR);
    // let created = CreateProcessW(
    //     PCWSTR(std::ptr::null()),
    //     PWSTR(cmdline.as_mut_ptr() as *mut _),
    //     std::ptr::null(),
    //     std::ptr::null(),
    //     BOOL(0),
    //     windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0),
    //     std::ptr::null(),
    //     PCWSTR(std::ptr::null()),
    //     &si,
    //     &mut pi
    // );

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
    RevertToSelf();

    // CreateProcessWithTokenW works but new process doesn't have foreground.
    // let created = CreateProcessWithTokenW(
    //     new_token,
    //     CREATE_PROCESS_LOGON_FLAGS(0),
    //     PCWSTR(std::ptr::null()),
    //     PWSTR(cmdline.as_mut_ptr() as *mut _),
    //     0,
    //     std::ptr::null(),
    //     PCWSTR(std::ptr::null()),
    //     &si,
    //     &mut pi
    // );

    CloseHandle(new_token);
    CloseHandle(token);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    if !created.as_bool() {
        crate::trace!("start", log::Level::Info, "Start app: CreateProcessAsUserW {:?} {:?}", Error::from_win32(), new_token);
        return Err(Error::from_win32());
    }

    return Ok(pi.dwProcessId);

}

// Using the shell token.
pub unsafe fn create_medium_process_shell(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOEXW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

    let mut self_token = HANDLE(0);
    if !OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ADJUST_GROUPS | TOKEN_ADJUST_PRIVILEGES,
        &mut self_token).as_bool()
    {
        return Err(Error::from_win32());
    }

    if !set_privilege(self_token, "SeIncreaseQuotaPrivilege".into(), true) {
        crate::trace!("start", log::Level::Info, "Start app: set_privilege {:?}", Error::from_win32());
        return Err(Error::from_win32());
    }
    CloseHandle(self_token);

    let shell = windows::Win32::UI::WindowsAndMessaging::GetShellWindow();
    
    let mut shell_pid: u32 = 0;
    windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(shell, &mut shell_pid);

    let processh = OpenProcess(PROCESS_QUERY_INFORMATION, BOOL(0), shell_pid);

    let mut token = HANDLE(0);
    if !OpenProcessToken(
        processh,
        TOKEN_DUPLICATE | TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
        &mut token).as_bool()
    {
        CloseHandle(processh);
        return Err(Error::from_win32());
    }
    CloseHandle(processh);

    let mut new_token = HANDLE(0);
    if !DuplicateTokenEx(token,
        TOKEN_ACCESS_MASK(395),
        // TOKEN_ALL_ACCESS,
        std::ptr::null(),
        SecurityImpersonation,
        TokenPrimary,
        &mut new_token).as_bool()
    {
        CloseHandle(token);
        return Err(Error::from_win32());
    }

    let mut proc_attr_size = 0usize;
    InitializeProcThreadAttributeList(
        LPPROC_THREAD_ATTRIBUTE_LIST(std::ptr::null_mut()),
        1,
        0,
        &mut proc_attr_size
    );

    let proc_attr_buf_layout = std::alloc::Layout::from_size_align(proc_attr_size, 1).unwrap();
    let proc_attr_buf = LPPROC_THREAD_ATTRIBUTE_LIST(std::alloc::alloc(proc_attr_buf_layout) as _);

    InitializeProcThreadAttributeList(
        proc_attr_buf,
        1,
        0,
        &mut proc_attr_size
    );

    let none: *const core::ffi::c_void = std::ptr::null();
    UpdateProcThreadAttribute(
        proc_attr_buf,
        0,
        PROC_THREAD_ATTRIBUTE_PARENT_PROCESS.try_into().unwrap(),
        std::mem::transmute::<_, *const core::ffi::c_void>(&none),
        std::mem::size_of::<core::ffi::c_void>(),
        std::ptr::null_mut(),
        std::ptr::null()
    );
    
    si.lpAttributeList = proc_attr_buf;

    SetLastError(NO_ERROR);
    let created = CreateProcessAsUserW(
        new_token,
        PCWSTR(std::ptr::null()),
        PWSTR(cmdline.as_mut_ptr() as *mut _),
        std::ptr::null(),
        std::ptr::null(),
        BOOL(0),
        EXTENDED_STARTUPINFO_PRESENT.0,
        std::ptr::null(),
        PCWSTR(std::ptr::null()),
        std::mem::transmute(&si),
        &mut pi
    );

    // CreateProcessWithTokenW works but new process doesn't have foreground.
    // let created = CreateProcessWithTokenW(
    //     new_token,
    //     CREATE_PROCESS_LOGON_FLAGS(0),
    //     PCWSTR(std::ptr::null()),
    //     PWSTR(cmdline.as_mut_ptr() as *mut _),
    //     0,
    //     std::ptr::null(),
    //     PCWSTR(std::ptr::null()),
    //     &si,
    //     &mut pi
    // );

    std::alloc::dealloc(proc_attr_buf.0 as _, proc_attr_buf_layout);


    CloseHandle(new_token);
    CloseHandle(token);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    if !created.as_bool() {
        crate::trace!("start", log::Level::Info, "Start app: CreateProcessAsUserW {:?} {:?}", Error::from_win32(), new_token);
        return Err(Error::from_win32());
    }

    return Ok(pi.dwProcessId);

}

// it works except the the token we create has the elevated logon session which is
// different from the unelevated logon session and shellexecute will fail in noconsole
// probably because of this.
pub unsafe fn create_medium_process_crap(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut self_token = HANDLE(0);
    if !OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ADJUST_GROUPS | TOKEN_ADJUST_PRIVILEGES | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
        &mut self_token).as_bool()
    {
        return Err(Error::from_win32());
    }

    let mut restricted_token = HANDLE(0);
    if !CreateRestrictedToken(
        self_token,
        DISABLE_MAX_PRIVILEGE | LUA_TOKEN,
        &[],
        &[],
        &[],
        &mut restricted_token
    ).as_bool()
    {
        crate::trace!("start", log::Level::Info, "Start app: CreateRestrictedToken {:?}", Error::from_win32());
        CloseHandle(self_token);
        return Err(Error::from_win32());
    }

    
    let mut sid =  PSID::default();
    let medium_integrity = "S-1-16-8192\0".encode_utf16().collect::<Vec<u16>>();
    if !ConvertStringSidToSidW(PCWSTR(medium_integrity.as_ptr()), &mut sid).as_bool() {
        CloseHandle(self_token);
        CloseHandle(restricted_token);
        return Err(Error::from_win32());
    }

    let mut til = TOKEN_MANDATORY_LABEL::default();
    til.Label.Attributes = SE_GROUP_INTEGRITY as u32;
    til.Label.Sid = sid;

    if !SetTokenInformation(restricted_token,
        TokenIntegrityLevel,
        std::mem::transmute(&til),
        std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid)).as_bool()
    {
        LocalFree(std::mem::transmute(sid));
        CloseHandle(self_token);
        CloseHandle(restricted_token);
        return Err(Error::from_win32());
    }
    LocalFree(std::mem::transmute(sid));


    // windows::Win32::System::Diagnostics::Debug::DebugBreak();

    SetLastError(NO_ERROR);
    let created = CreateProcessAsUserW(
        restricted_token,
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

    CloseHandle(self_token);
    CloseHandle(restricted_token);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    if !created.as_bool() {
        crate::trace!("start", log::Level::Info, "Start app: CreateProcessAsUserW {:?} {:?}", Error::from_win32(), restricted_token);
        return Err(Error::from_win32());
    }

    return Ok(pi.dwProcessId);
}


// Use createrestricted token, but createprocessasuser with it fails..
pub unsafe fn create_medium_process_restricted_token(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut level = SAFER_LEVEL_HANDLE(0);
    SaferCreateLevel(
        SAFER_SCOPEID_USER,
        SAFER_LEVELID_NORMALUSER,
        SAFER_LEVEL_OPEN,   
        &mut level,
        std::ptr::null_mut());

    let mut safe_token = HANDLE(0);
    if !SaferComputeTokenFromLevel(
        level,
        HANDLE(0),
        &mut safe_token,
        SAFER_COMPUTE_TOKEN_FROM_LEVEL_FLAGS(0),
        std::ptr::null_mut()).as_bool()
    {
        // eh nothing for now.
    }

    SaferCloseLevel(level);

    
    let mut sid =  PSID::default();
    let medium_integrity = "S-1-16-8192\0".encode_utf16().collect::<Vec<u16>>();
    if !ConvertStringSidToSidW(PCWSTR(medium_integrity.as_ptr()), &mut sid).as_bool() {
        CloseHandle(safe_token);
        return Err(Error::from_win32());
    }

    let mut til = TOKEN_MANDATORY_LABEL::default();
    til.Label.Attributes = SE_GROUP_INTEGRITY as u32;
    til.Label.Sid = sid;

    if !SetTokenInformation(safe_token,
        TokenIntegrityLevel,
        std::mem::transmute(&til),
        std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid)).as_bool()
    {
        LocalFree(std::mem::transmute(sid));
        CloseHandle(safe_token);
        return Err(Error::from_win32());
    }

    let mut restricted_token = HANDLE(0);
    if !CreateRestrictedToken(
        safe_token,
        DISABLE_MAX_PRIVILEGE | LUA_TOKEN,
        &[],
        &[],
        &[],
        &mut restricted_token
    ).as_bool()
    {
        crate::trace!("start", log::Level::Info, "Start app: CreateRestrictedToken {:?}", Error::from_win32());
        LocalFree(std::mem::transmute(sid));
        CloseHandle(safe_token);
        return Err(Error::from_win32());
    }

    // windows::Win32::System::Diagnostics::Debug::DebugBreak();

    // https://stackoverflow.com/questions/49972136/adjusttokenprivileges-error-not-all-assigned-after-success
    // https://msdn.microsoft.com/en-us/library/windows/desktop/aa375202(v=vs.85).aspx
    // SE_PRIVILEGE_REMOVED:

    // Because the privilege has been removed from the token, attempts to reenable the privilege result in the warning ERROR_NOT_ALL_ASSIGNED as if the privilege had never existed.
    // https://stackoverflow.com/questions/18022053/adjusttokenprivileges-error-error-not-all-assigned
    // set_privilege gives ERROR_NOT_ALL_ASSIGNED
    // let mut self_token = HANDLE(0);
    // if !OpenProcessToken(
    //     GetCurrentProcess(),
    //     TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ADJUST_GROUPS | TOKEN_ADJUST_PRIVILEGES,
    //     &mut self_token).as_bool()
    // {
    //     LocalFree(std::mem::transmute(sid));
    //     CloseHandle(safe_token);
    //     return Err(Error::from_win32());
    // }

    // if !set_privilege(self_token, "SeAssignPrimaryTokenPrivilege".into(), true) {
    //     crate::trace!("start", log::Level::Info, "Start app: set_privilege {:?}", Error::from_win32());
    //     LocalFree(std::mem::transmute(sid));
    //     CloseHandle(safe_token);
    //     CloseHandle(self_token);
    //     return Err(Error::from_win32());
    // }
    // CloseHandle(self_token);

    SetLastError(NO_ERROR);
    let created = CreateProcessAsUserW(
        restricted_token,
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
    CloseHandle(safe_token);
    CloseHandle(restricted_token);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    if !created.as_bool() {
        crate::trace!("start", log::Level::Info, "Start app: CreateProcessAsUserW {:?} {:?}", Error::from_win32(), restricted_token);
        return Err(Error::from_win32());
    }

    return Ok(pi.dwProcessId);
}

// Debug with whoami /groups /fo list
// https://docs.microsoft.com/en-us/windows/security/identity-protection/access-control/security-identifiers
// created token still has this group
// S-1-5-114
pub unsafe fn create_medium_process3(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut level = SAFER_LEVEL_HANDLE(0);
    SaferCreateLevel(
        SAFER_SCOPEID_USER,
        SAFER_LEVELID_NORMALUSER,
        SAFER_LEVEL_OPEN,   
        &mut level,
        std::ptr::null_mut());

    let mut restricted_token = HANDLE(0);
    if !SaferComputeTokenFromLevel(
        level,
        HANDLE(0),
        &mut restricted_token,
        SAFER_COMPUTE_TOKEN_FROM_LEVEL_FLAGS(0),
        std::ptr::null_mut()).as_bool()
    {
        // eh nothing for now.
    }

    SaferCloseLevel(level);

    
    let mut sid =  PSID::default();
    let medium_integrity = "S-1-16-8192\0".encode_utf16().collect::<Vec<u16>>();
    if !ConvertStringSidToSidW(PCWSTR(medium_integrity.as_ptr()), &mut sid).as_bool() {
        CloseHandle(restricted_token);
        return Err(Error::from_win32());
    }

    let mut til = TOKEN_MANDATORY_LABEL::default();
    til.Label.Attributes = SE_GROUP_INTEGRITY as u32;
    til.Label.Sid = sid;

    if !SetTokenInformation(restricted_token,
        TokenIntegrityLevel,
        std::mem::transmute(&til),
        std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid)).as_bool()
    {
        LocalFree(std::mem::transmute(sid));
        CloseHandle(restricted_token);
        return Err(Error::from_win32());
    }

    SetLastError(NO_ERROR);
    let created = CreateProcessAsUserW(
        restricted_token,
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
    CloseHandle(restricted_token);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    if !created.as_bool() {
        return Err(Error::from_win32());
    }

    return Ok(pi.dwProcessId);
}


// This method uses AdjustTokenGroups to remove elevated sids...
// Actually AdjustTokenGroups can't change mandatory groups SE_GROUP_MANDATORY, which admin s-1-5-32-544 is if you look at
// whoami /all
pub unsafe fn create_medium_process2(cmdline: String) -> Result<u32> {
    let mut cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut token = HANDLE(0);
    if !OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_DUPLICATE | TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY | TOKEN_ADJUST_GROUPS,
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

    let mut prev_length = 0u32;
    // AdjustTokenGroups(
    //     new_token,
    //     BOOL(1),
    //     std::ptr::null(),
    //     0,
    //     std::ptr::null_mut(),
    //     std::ptr::null_mut(),
    // );

    AdjustTokenGroups(
        new_token,
        BOOL(0),
        std::ptr::null(),
        0,
        std::ptr::null_mut(),
        &mut prev_length,
    );

    let group_buf_layout = std::alloc::Layout::from_size_align(prev_length as usize, 1).unwrap();
    let group_buf = std::alloc::alloc(group_buf_layout) as *mut TOKEN_GROUPS;

    AdjustTokenGroups(
        new_token,
        BOOL(0),
        std::ptr::null(),
        0,
        group_buf,
        &mut prev_length,
    );


    // from ntifs.h
    let nt_authority = SID_IDENTIFIER_AUTHORITY {
        Value: [0, 0, 0, 0, 0, 5]
    };
    let mut admin_sid = PSID(0);

    // Administrators is S-1-5-32-544
    AllocateAndInitializeSid(
        &nt_authority,
        2,
        SECURITY_BUILTIN_DOMAIN_RID as u32,
        DOMAIN_ALIAS_RID_ADMINS as u32,
        0,
        0,
        0,
        0,
        0,
        0,
        &mut admin_sid);

    // let mut sid_length = 0u32;
    // LookupAccountNameA(
    //     PCSTR(std::ptr::null()),
    //     PCSTR("administrators\0".as_ptr()),
    //     // std::mem::transmute(std::ptr::null_mut()),
    //     PSID(0),
    //     &mut sid_length,
    //     PSTR(std::ptr::null_mut()),
    //     std::ptr::null_mut(),
    //     std::ptr::null_mut(),
    // );

    // let sid_buf_layout = std::alloc::Layout::from_size_align(sid_length as usize, 1).unwrap();
    // let sid_buf = std::mem::transmute::<_, PSID>(std::alloc::alloc(sid_buf_layout));

    // // This crashes for some reason maybe debug this.
    // LookupAccountNameA(
    //     PCSTR(std::ptr::null()),
    //     PCSTR("administrators\0".as_ptr()),
    //     sid_buf,
    //     &mut sid_length,
    //     PSTR(std::ptr::null_mut()),
    //     std::ptr::null_mut(),
    //     std::ptr::null_mut(),
    // );

    let group_buf = group_buf.as_mut().unwrap();
    for i in 0 .. group_buf.GroupCount as usize {
        let mut groups: &mut [SID_AND_ATTRIBUTES] = std::slice::from_raw_parts_mut(group_buf.Groups.as_mut_ptr(), group_buf.GroupCount as usize);
        if EqualSid(groups[i].Sid, admin_sid).as_bool() {
            groups[i].Attributes = CLAIM_SECURITY_ATTRIBUTE_USE_FOR_DENY_ONLY.0;
        }
    }
    
    AdjustTokenGroups(
        new_token,
        BOOL(0),
        group_buf,
        prev_length,
        group_buf,
        &mut prev_length,
    );

    let group_buf = group_buf as *mut TOKEN_GROUPS as *mut u8;
    std::alloc::dealloc(group_buf, group_buf_layout);

    // let sid_buf = std::mem::transmute::<_, *mut u8>(sid_buf);
    // std::alloc::dealloc(sid_buf, sid_buf_layout);
    FreeSid(admin_sid);

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

// This creates a process with medium integrity
// which fixes some shell integration issues but
// the process still has admin groups.
pub unsafe fn create_medium_process_simple(cmdline: String) -> Result<u32> {
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
