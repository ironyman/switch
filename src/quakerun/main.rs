#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::alloc::GlobalAlloc;

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
    Win32::System::WindowsProgramming::*,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Dwm::*,
    Win32::System::Diagnostics::ToolHelp::*,
    Win32::System::SystemServices::*,
    Win32::System::Console::*,
    Win32::System::Diagnostics::Debug::*,
};

use clap::{Arg, Command};

#[path="../windows2.rs"]
mod windows2;

const WAIT_QUAKE_SECONDS: u32 = 60;
const QUAKE_HOT_KEY_ID: i32 = 1;
const QUAKE_WIN_HOT_KEY_ID: i32 = 2;
const OPEN_QUAKE_EVENT_NAME: &str = "OpenQuake";
const HIDE_QUAKE_EVENT_NAME: &str = "HideQuake";
const EXIT_QUAKE_EVENT_NAME: &str = "ExitQuake";
const RUN_QUAKE_EVENT_NAME: &str = "RunQuake";

unsafe fn create_process(cmdline: String) -> Result<u32> {
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

unsafe fn create_initial_quake_window(command: &str) -> Result<HWND> {
    let cmdline = "wt -w _quake ".to_string() 
        + &format!("{} --runner -c \"{}\"", std::env::current_exe().unwrap().to_str().unwrap(), command);
    println!("Running {}", cmdline);
    log::trace!("[{}] Running {}", GetCurrentProcessId(), cmdline);

    let pid = create_process(cmdline)?;
    return Ok(wait_for_quake_window_start(pid)?);
}

unsafe fn wait_for_quake_window_start(process_id: u32) -> Result<HWND> {
    let start_time = std::time::SystemTime::now();

    while start_time.elapsed().unwrap().as_secs() < WAIT_QUAKE_SECONDS as u64 {
        Sleep(500);
        let windows_terminal_pid = get_child_pid(process_id);
        if windows_terminal_pid == u32::MAX {
            continue;
        }

        println!("Found windowsterminal.exe pid: {}", windows_terminal_pid);
        log::trace!("[{}] Found windowsterminal.exe pid: {}", GetCurrentProcessId(), windows_terminal_pid);

        let hwnd = get_process_window(windows_terminal_pid).unwrap();

        if !hwnd.is_invalid() {
            set_dwm_style(hwnd)?;

            // Wait for window to appear.
            while !IsWindowVisible(hwnd).as_bool() && start_time.elapsed().unwrap().as_secs() < WAIT_QUAKE_SECONDS as u64 {
                Sleep(5);
            }

            // Hide it
            while IsWindowVisible(hwnd).as_bool() && start_time.elapsed().unwrap().as_secs() < WAIT_QUAKE_SECONDS as u64 {
                log::trace!("[{}] Hiding window windowsterminal", GetCurrentProcessId());

                // ShowWindow(hwnd, SW_HIDE);
                // ShowWindow fails sometimes...

                if !SetWindowPos( hwnd, HWND(0), 0, 0, 0, 0, 
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_HIDEWINDOW).as_bool() {
                    log::trace!("[{}] Hiding window failed {}", GetCurrentProcessId(), GetLastError().0);
                }
                // ShowWindow(hwnd, SW_MINIMIZE);
            }

            return Ok(hwnd);
        }
    }

    return Ok(HWND(0));
}

unsafe fn get_child_pid(pid: u32) -> u32 {
    let mut pe32: PROCESSENTRY32 = std::mem::zeroed();

    let mut child_pid: u32 = u32::MAX;

    let snapshot = CreateToolhelp32Snapshot( TH32CS_SNAPPROCESS, 0 );
    if snapshot == INVALID_HANDLE_VALUE {
        return u32::MAX
    }

    pe32.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;
    if !Process32First(snapshot, &mut pe32).as_bool() {
        CloseHandle(snapshot);
        return u32::MAX
    }

    loop {
        if pe32.th32ParentProcessID == pid {
            child_pid = pe32.th32ProcessID;
            break;
        }
        if !Process32Next(snapshot, &mut pe32).as_bool() {
            break;
        }
    }

    CloseHandle(snapshot);

    return child_pid;
}

struct EnumWindowData {
    hwnd: HWND,
    process_id: u32,
}

unsafe fn get_process_window(process_id: u32) -> Result<HWND> {
    // let mut hwnd: HWND = std::mem::zeroed::<HWND>();
    let mut data = EnumWindowData {
        hwnd: HWND(0),
        process_id,
    };

    EnumWindows(Some(enum_window_proc), LPARAM(&mut data as *mut _ as isize));
    Ok(data.hwnd)
}

unsafe extern "system" fn enum_window_proc(windowh: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(windowh).as_bool() {
        return BOOL(1);
    }
    
    let lparam = lparam.0 as usize as *mut EnumWindowData;

    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(windowh, &mut process_id);
    if process_id == (*lparam).process_id {
        (*lparam).hwnd = windowh;
        return BOOL(0);
    }

    return BOOL(1);
}

unsafe fn set_dwm_style(window: HWND) -> Result<()> {
    let corner_preference = DWMWCP_DONOTROUND;
    DwmSetWindowAttribute(
        window, 
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner_preference as *const _ as *const core::ffi::c_void,
        std::mem::size_of_val(&corner_preference) as u32)?;

    // When specifying an explicit RGB color, the COLORREF value has the following hexadecimal form:
    // 0x00bbggrr

    let border_color = 0 as u32;
    DwmSetWindowAttribute(
        window,
        DWMWA_BORDER_COLOR,
        &border_color  as *const _ as *const core::ffi::c_void, 
        std::mem::size_of_val(&border_color) as u32)?;

    let disable_animiation = BOOL(1);
    DwmSetWindowAttribute(
        window, 
        DWMWA_TRANSITIONS_FORCEDISABLED,
        core::mem::transmute(&disable_animiation),
        std::mem::size_of_val(&disable_animiation) as u32)?;

    // Making the quake window a tool window will disable animation
    // but it creates a minized window outside of taskbar.
    // let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
    // SetWindowLongPtrW(window, GWL_EXSTYLE, style | (WS_EX_TOOLWINDOW.0 as isize));
    
    return Ok(());
}

unsafe extern "system" fn ctrl_handler(ctrltype: u32) -> BOOL {
    if ctrltype == CTRL_C_EVENT {
        println!("Ctrl-c hit, exiting");
        //DebugBreak();
        set_event_by_name(EXIT_QUAKE_EVENT_NAME);
    }
    return BOOL(1);
}

unsafe fn kill_window_process(windowh: HWND) {
    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(windowh, &mut process_id);
    let processh = OpenProcess(PROCESS_TERMINATE, BOOL(0), process_id);
    TerminateProcess(processh, 1);
    CloseHandle(processh);
}

fn set_event_by_name(event_name: &str) {
    let event_name: Vec<u16> = (event_name.to_string() + "\0").encode_utf16().collect::<Vec<u16>>();

    unsafe {
        let event = OpenEventW(THREAD_SYNCHRONIZE.0 | EVENT_MODIFY_STATE , BOOL(0), PCWSTR(event_name.as_ptr() as *const _));
        assert!(!event.is_invalid());
        SetEvent(event);
        CloseHandle(event);
    }
}

fn quake_terminal_runner(command: &str) -> Result<()> {
    unsafe {
        let should_exit_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(EXIT_QUAKE_EVENT_NAME)
        );

        let run_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(RUN_QUAKE_EVENT_NAME)
        );

        let events = [run_event, should_exit_event];
        const RUN_WAIT: u32 = WAIT_OBJECT_0;
        const EXIT_WAIT: u32 = WAIT_OBJECT_0 + 1;

        loop {
            match WaitForMultipleObjects(&events, BOOL(0), INFINITE) {
                RUN_WAIT => {
                    let pid = create_process(command.into());

                    let pid = if pid.is_err() {
                        set_event_by_name(HIDE_QUAKE_EVENT_NAME);
                        ResetEvent(run_event);
                        continue
                    } else {
                        pid.unwrap()
                    };

                    let processh = OpenProcess(PROCESS_SYNCHRONIZE, BOOL(0), pid);
                    WaitForSingleObject(processh, INFINITE);
                    CloseHandle(processh);

                    set_event_by_name(HIDE_QUAKE_EVENT_NAME);
                    ResetEvent(run_event);
                },
                EXIT_WAIT => {
                    break;
                }
                _ => {
                    assert!(false, "Unexpected WaitForMultipleObjects failure");
                }
            }
        }
        return Ok(());
    }
}

fn main() -> Result<()> {
    eventlog::register("switch").unwrap();
    eventlog::init("switch", log::Level::Trace).unwrap();
    log::trace!("[{}] Quakerun started", unsafe { GetCurrentProcessId() });

    let matches = Command::new("quakerun")
        .arg(Arg::new("runner")
            .short('r')
            .long("runner")
            .help("Run as server in quake terminal"))
        .arg(Arg::new("open")
            .short('o')
            .long("open"))
        .arg(Arg::new("hide")
            .short('h')
            .long("hide")
            .help("Hide quake terminal"))
        .arg(Arg::new("stop")
            .short('s')
            .long("stop")
            .help("Stop quake runner"))
        .arg(Arg::new("command")
            .short('c')
            .long("command")
            .help("Command to run")
            .value_name("COMMAND")
            // .required(true)
            .takes_value(true))
        .get_matches();

    if matches.occurrences_of("open") == 1 {
        set_event_by_name(OPEN_QUAKE_EVENT_NAME);
        return Ok(());
    } else if matches.occurrences_of("hide") == 1 {
        set_event_by_name(HIDE_QUAKE_EVENT_NAME);
        return Ok(());
    } else if matches.occurrences_of("stop") == 1 {
        set_event_by_name(EXIT_QUAKE_EVENT_NAME);
        return Ok(());
    }

    if matches.value_of("command").is_none() {
        println!("Need --command to be specified.");
        return Err(Error::from(ERROR_INVALID_PARAMETER));
    }

    if matches.occurrences_of("runner") == 1 {
        log::trace!("[{}] Quakerun started as terminal runner", unsafe { GetCurrentProcessId() });
        return quake_terminal_runner(matches.value_of("command").unwrap());
    }

    unsafe {  
        SetLastError(NO_ERROR);
        let open_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(OPEN_QUAKE_EVENT_NAME)
        );

        assert!(!open_quake_event.is_invalid());

        if Error::from_win32().code() == ERROR_ALREADY_EXISTS.to_hresult() {    
            return Ok(());
        }

        let hide_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(HIDE_QUAKE_EVENT_NAME)
        );

        assert!(!hide_quake_event.is_invalid());

        if Error::from_win32().code() == ERROR_ALREADY_EXISTS.to_hresult() {    
            return Ok(());
        }

        let should_exit_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(EXIT_QUAKE_EVENT_NAME)
        );
        
        assert!(!should_exit_event.is_invalid());

        if Error::from_win32().code() == ERROR_ALREADY_EXISTS.to_hresult() {    
            return Ok(());
        }

        let run_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(RUN_QUAKE_EVENT_NAME)
        );

        assert!(!run_quake_event.is_invalid());

        if Error::from_win32().code() == ERROR_ALREADY_EXISTS.to_hresult() {    
            return Ok(());
        }

        // Prevent this instance of quake terminal from registering default quake terminal hotkey.
        RegisterHotKey(HWND(0), QUAKE_WIN_HOT_KEY_ID, MOD_WIN | MOD_NOREPEAT, VK_OEM_3.0 as u32);

        let quake_window = create_initial_quake_window(matches.value_of("command").unwrap())?;
        println!("Found quake window hwnd {:?}", quake_window);
        UnregisterHotKey(HWND(0), QUAKE_WIN_HOT_KEY_ID);

        // backtick
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        RegisterHotKey(HWND(0), QUAKE_HOT_KEY_ID, MOD_ALT | MOD_NOREPEAT, VK_OEM_3.0 as u32);

        SetConsoleCtrlHandler(Some(ctrl_handler), BOOL(1));

        let mut msg: MSG = std::mem::zeroed();

        loop {
            let events =  [open_quake_event, hide_quake_event, should_exit_event];
            const OPEN_WAIT: u32 = WAIT_OBJECT_0;
            const HIDE_WAIT: u32 = WAIT_OBJECT_0 + 1;
            const EXIT_WAIT: u32 = WAIT_OBJECT_0 + 2;
            let message_wait = WAIT_OBJECT_0 + events.len() as u32;
            
            match MsgWaitForMultipleObjects(&events, BOOL(0), INFINITE, QS_ALLINPUT) {
                OPEN_WAIT => {
                    println!("WaitForMultipleObjects: OPEN_WAIT");

                    if !IsWindowVisible(quake_window).as_bool() {
                        SetEvent(run_quake_event);
                        ShowWindow(quake_window, SW_SHOW);
                    }
                    
                    // windows2::set_foreground_window_ex(quake_window);
                    
                    let cmdline = "wt -w _quake fp --target 0".to_string();
                    let pid = create_process(cmdline)?;

                    ResetEvent(open_quake_event);
                },
                HIDE_WAIT => {
                    println!("WaitForMultipleObjects: HIDE_WAIT");
                    ShowWindow(quake_window, SW_HIDE);
                    ShowWindow(quake_window, SW_MINIMIZE);
                    ResetEvent(hide_quake_event);
                },
                EXIT_WAIT => {
                    break;
                }
                wait => {
                    assert!(wait == message_wait, "Unexpected WaitForMultipleObjects failure");
                    while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                        match msg.message {
                            WM_HOTKEY => {
                                // println!("Hotkey pressed!");
                                
                                // if !IsWindowVisible(quake_window).as_bool() {
                                if WaitForSingleObject(run_quake_event, 0) != WAIT_OBJECT_0 {
                                    SetEvent(run_quake_event);
                                    ShowWindow(quake_window, SW_SHOW);
                                }
                                
                                windows2::set_foreground_window_terminal(quake_window)?;
                                // windows2::set_foreground_window_ex(quake_window);
                                // SetEvent(open_quake_event);
                            },
                            _ => {
                                TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }
                        }
                    }
                }
            }
        }

        UnregisterHotKey(HWND(0), QUAKE_HOT_KEY_ID);
        
        CloseHandle(hide_quake_event);
        CloseHandle(open_quake_event);

        DestroyWindow(quake_window); // Doesn't work..
        kill_window_process(quake_window);

        // eventlog::deregister("switch").unwrap();

        Ok(())
    }
}