use std::io::Write;
use switch::ListContentProvider;

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Dwm::*,
    Win32::Graphics::Gdi::*,
    Win32::System::Diagnostics::ToolHelp::*,
    Win32::System::SystemServices::*,
    Win32::System::SystemInformation::*,
    Win32::System::Console::*,
    Win32::Storage::FileSystem::*,
    Win32::Security::Authorization::*,
    Win32::Security::*,
    Win32::System::IO::*,
    Win32::System::Memory::*,
    Win32::System::Pipes::*,
};

use clap::{Arg, Command};
use switch::windowprovider;
use switch::setforegroundwindow::set_foreground_window_terminal;
use switch::waitlist::{WaitList, WaitResult};

// Weird you have to import like this to use macro trace!, fully qualified path doesn't work,
// but after you import it, its path becomes switch::trace! even though its full path is under switch::log...
use switch::log::*;

const WAIT_QUAKE_SECONDS: u32 = 60;
const QUAKE_HOT_KEY_ID: i32 = 1;
const QUAKE_WIN_HOT_KEY_ID: i32 = 2;

const OPEN_QUAKE_EVENT_NAME: &str = "OpenQuake";
const HIDE_QUAKE_EVENT_NAME: &str = "HideQuake";
const EXIT_QUAKE_EVENT_NAME: &str = "ExitQuake";
const BTM_EVENT_NAME: &str = "BTM";
const RUN_QUAKE_EVENT_NAME: &str = "RunQuake";

// const WM_START_SWITCH: u32 = WM_USER + 1;

static mut HOOK_HANDLE: HHOOK = HHOOK(0);
static mut START_SWITCH_WRITE: HANDLE = HANDLE(0);
// static mut MAIN_THREAD_ID: u32 = 0u32;

unsafe fn create_initial_quake_window(command: &str) -> Result<HWND> {
    let cmdline = "wt -w _quake ".to_string() 
        + &format!("{} --runner -c \"{}\"", std::env::current_exe().unwrap().to_str().unwrap(), command);
    println!("Running {}", cmdline);

    let pid = switch::create_process::create_process(cmdline)?;
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

        let hwnd = get_process_window(windows_terminal_pid).unwrap();

        if !hwnd.is_invalid() {
            set_dwm_style(hwnd)?;

            // Wait for window to appear.
            while !IsWindowVisible(hwnd).as_bool() && start_time.elapsed().unwrap().as_secs() < WAIT_QUAKE_SECONDS as u64 {
                Sleep(5);
            }

            // // Hide it
            // while IsWindowVisible(hwnd).as_bool() && start_time.elapsed().unwrap().as_secs() < WAIT_QUAKE_SECONDS as u64 {
            //     log::trace!("[{}] Hiding window windowsterminal", GetCurrentProcessId());

            //     // ShowWindow(hwnd, SW_HIDE);
            //     // ShowWindow fails sometimes...

            //     if !SetWindowPos( hwnd, HWND(0), 0, 0, 0, 0, 
            //         SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_HIDEWINDOW).as_bool() {
            //         log::trace!("[{}] Hiding window failed {}", GetCurrentProcessId(), GetLastError().0);
            //     }
            //     // ShowWindow(hwnd, SW_MINIMIZE);
            // }

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
    let disable_animation = BOOL(1);
    DwmSetWindowAttribute(
        window, 
        DWMWA_TRANSITIONS_FORCEDISABLED,
        core::mem::transmute(&disable_animation),
        std::mem::size_of_val(&disable_animation) as u32)?;

    // When specifying an explicit RGB color, the COLORREF value has the following hexadecimal form:
    // 0x00bbggrr

    let border_color = 0 as u32;
    DwmSetWindowAttribute(
        window,
        DWMWA_BORDER_COLOR,
        &border_color  as *const _ as *const core::ffi::c_void, 
        std::mem::size_of_val(&border_color) as u32)?;

    let corner_preference = DWMWCP_DONOTROUND;
    DwmSetWindowAttribute(
        window, 
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner_preference as *const _ as *const core::ffi::c_void,
        std::mem::size_of_val(&corner_preference) as u32)?;

    // Making the quake window a tool window will disable animation
    // but it creates a minized window outside of taskbar.
    // let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
    // SetWindowLongPtrW(window, GWL_EXSTYLE, style | (WS_EX_TOOLWINDOW.0 as isize));

    return Ok(());
}

unsafe extern "system" fn ctrl_handler(ctrltype: u32) -> BOOL {
    if ctrltype == CTRL_C_EVENT {
        // println!("Ctrl-c hit, exiting");
        //DebugBreak();
        // set_event_by_name(EXIT_QUAKE_EVENT_NAME);
    }
    return BOOL(1);
}

pub unsafe extern "system" fn destroy_highlight_window(_instance: *mut TP_CALLBACK_INSTANCE, context: *mut ::core::ffi::c_void, _timer: *mut TP_TIMER) {
    let highlight_window = core::mem::transmute::<_, HWND>(context);
    switch::trace!("highlight_window", log::Level::Debug, "destroy_highlight_window: {:?}", highlight_window);
    // DestroyWindow(highlight_window);
    SendMessageW(highlight_window, WM_CLOSE, WPARAM(0), LPARAM(0));    
}

pub unsafe extern "system" fn create_highlight_window(_instance: *mut TP_CALLBACK_INSTANCE, context: *mut ::core::ffi::c_void, _timer: *mut TP_TIMER) {
    let target_window = core::mem::transmute::<_, HWND>(context);
    let instance = windows::Win32::System::LibraryLoader::GetModuleHandleA(None);

    // This causes windows to hang when switching rapidly.
    // AttachThreadInput(GetCurrentThreadId(), MAIN_THREAD_ID, BOOL(1));

    extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match message as u32 {
                // Both WM_SHOWWINDOW and WM_CREATE is before window is shown.
                // WM_SHOWWINDOW => {
                //     // SetTimer(window, 1, 400, None);
                //     LRESULT(0)
                // }
                // not used.
                // 1 => {
                //     switch::trace!("highlight_window", log::Level::Debug, "wndproc: timer expired");
                //     DestroyWindow(window);
                //     LRESULT(0)
                // }
                WM_PAINT => {
                    ValidateRect(window, std::ptr::null());
                    LRESULT(0)
                }
                WM_NCPAINT => {
                    ValidateRect(window, std::ptr::null());
                    LRESULT(0)
                }
                WM_DESTROY => {
                    switch::trace!("highlight_window", log::Level::Debug, "wndproc: destroy");
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                _ => DefWindowProcA(window, message, wparam, lparam),
            }
        }
    }
    let window_class = PCSTR(b"highlightwindow\0".as_ptr());
    let wc = WNDCLASSA {
        hCursor: LoadCursorW(None, IDC_ARROW),
        hInstance: instance,
        lpszClassName: window_class,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hbrBackground: core::mem::transmute(GetStockObject(BLACK_BRUSH)),
        ..Default::default()
    };
    
    let _atom = RegisterClassA(&wc);

    let mut rc: RECT = std::mem::zeroed();
    GetWindowRect(target_window, &mut rc);

    let highlight_window = CreateWindowExA(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
        window_class,
        PCSTR(std::ptr::null()),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_POPUP, // WS_POPUP removes NC area, but WS_OVERLAPPEDWINDOW adds it back...
        rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top,
        None, None, instance, std::ptr::null());
	SetLayeredWindowAttributes(highlight_window, 0, 30, LWA_ALPHA);
	UpdateWindow(highlight_window);

    switch::trace!("highlight_window", log::Level::Debug, "create_highlight_window: {:?}", highlight_window);

    ShowWindow(highlight_window, SW_SHOW);

    let timer = CreateThreadpoolTimer(Some(destroy_highlight_window), core::mem::transmute(highlight_window), std::ptr::null());
    let mut clear_time = FILETIME::default();
    
    GetSystemTimeAsFileTime(&mut clear_time);
    clear_time.dwLowDateTime += 10*1000*100;
    SetThreadpoolTimer(timer, &clear_time, 0, 0);

    let mut message = MSG::default();

    while GetMessageA(&mut message, HWND(0), 0, 0).into() {
        DispatchMessageA(&message);
    }

    UnregisterClassA(wc.lpszClassName, instance);
}

// Use create_highlight_window instead.
unsafe extern "system" fn _toggle_highlight(_instance: *mut TP_CALLBACK_INSTANCE, context: *mut ::core::ffi::c_void, _timer: *mut TP_TIMER) {
    let window = core::mem::transmute::<_, HWND>(context);
    if GetForegroundWindow() == window {
        switch::trace!("directional_switching", log::Level::Debug, "toggle_highlight: {:?}", window);
        switch::windowgeometry::highlight_window(window);
    }
}

// Capslock is modifier key for CAP + arrow shortcuts.
// Shift + CAP is used to toggle capslock.
unsafe extern "system" fn low_level_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    static mut CAPSLOCK_PRESSED: bool = false;
    static mut SHIFT_PRESSED: bool = false;
    switch::trace!("hotkey", log::Level::Info, "Enter low_level_keyboard_proc");

    if code < 0 || code != HC_ACTION as i32 {
        switch::trace!("hotkey", log::Level::Info, "Immediately CallNextHookEx");
        return CallNextHookEx(HOOK_HANDLE, code, wparam, lparam);
    }

    let kbdllhookstruct: *const KBDLLHOOKSTRUCT = lparam.0 as *const _;
    let vk = VIRTUAL_KEY((*kbdllhookstruct).vkCode as u16);
    let press_state = wparam.0 as u32;

    let mut key_text: [u8; 512] = [0; 512];
    let vsc = MapVirtualKeyA(vk.0.into(), MAPVK_VK_TO_VSC);
    let _key_text_len = GetKeyNameTextA((vsc << 16) as i32, &mut key_text);
    switch::trace!("hotkey", log::Level::Info, "low_level_keyboard_proc key {}, {}, {}", vk.0, press_state, std::str::from_utf8(&key_text).unwrap().trim_matches(char::from(0)));

    if vk == VK_SHIFT || vk == VK_LSHIFT || vk == VK_RSHIFT  {
        if press_state == WM_KEYDOWN {
            switch::trace!("hotkey", log::Level::Info, "Shift pressed");
            SHIFT_PRESSED = true;
        } else {
            switch::trace!("hotkey", log::Level::Info, "Shift released");
            SHIFT_PRESSED = false;
        }
    }

    if vk == VK_CAPITAL && !SHIFT_PRESSED {
        if press_state == WM_KEYDOWN {
            switch::trace!("hotkey", log::Level::Info, "Caps lock pressed");
            CAPSLOCK_PRESSED = true;
        } else {
            switch::trace!("hotkey", log::Level::Info, "Caps lock released");
            CAPSLOCK_PRESSED = false;
        }
        return LRESULT(1);
    }

    if CAPSLOCK_PRESSED {
        switch::trace!("hotkey", log::Level::Info, "Caps lock modifier actions");
        if press_state == WM_KEYDOWN && vk == VK_P {
            switch::trace!("hotkey", log::Level::Info, "CAPS + P pressed");
            std::thread::spawn(move || {
                let arg = "--mode startapps";
                // let layout = std::alloc::Layout::from_size_align(arg.len(), 1).unwrap();
                // let buf = std::alloc::alloc(layout);
                // unsafe { PostThreadMessageA(MAIN_THREAD_ID, WM_START_SWITCH, WPARAM(buf as usize), LPARAM(0)); }
                switch::trace!("hotkey", log::Level::Info, "cap + p pressed");
                let mut written = 0u32;
                WriteFile(
                    START_SWITCH_WRITE,
                    arg.as_bytes().as_ptr() as _,
                    arg.len() as u32,
                    &mut written,
                    std::ptr::null_mut());
                assert!(written as usize == arg.len());
                switch::trace!("hotkey", log::Level::Info, "cap + p pressed wrote to pipe");
                return;
            });
        } else if press_state == WM_KEYDOWN && vk == VK_RETURN {
            // This is not what WindowProvider is meant to be used for
            // but I need a list of windows excluding the quakerun host term window.
            // Which is what WindowProvider does. Maybe fix this later.
            let mut wp = switch::WindowProvider::new();
            let windows = wp.query_for_items();
            let terminals: Vec<&switch::windowprovider::WindowInfo> = windows.iter().map(|w| {
                (*w).as_any().downcast_ref::<switch::windowprovider::WindowInfo>().unwrap()
            }).filter(|&w| {
                w.image_name == "WindowsTerminal"
            }).collect();

            if terminals.len() == 0 {
                let wt_path = "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\wt.exe\0";
                let mut expanded: [u8; 512] = [0; 512];
                let len = windows::Win32::System::Environment::ExpandEnvironmentStringsA(PCSTR(wt_path.as_ptr()), &mut expanded) as usize;
                let cmdline = String::from_utf8_lossy(&expanded[..len-1]).into();
                let _  = switch::create_process::create_process(cmdline);
            } else {
                let current = terminals.iter().position(|&t| t.windowh == GetForegroundWindow());
                let next = match current {
                    Some(index) => {
                        (index + 1) % terminals.len()
                    },
                    _ => 0
                };
                set_foreground_window_terminal(terminals[next].windowh).ok();
            }
        } else if press_state == WM_KEYDOWN && vk == VK_O {
            set_event_by_name(BTM_EVENT_NAME);
        } else if press_state == WM_KEYUP {
            std::thread::spawn(move || {
                let current = GetForegroundWindow();

                let adjacent_window = match vk {
                    VK_LEFT => {
                        switch::windowgeometry::get_adjacent_window(
                            current,
                            switch::windowgeometry::Direction::Left)
                    },
                    VK_RIGHT => {
                        switch::windowgeometry::get_adjacent_window(
                            current,
                            switch::windowgeometry::Direction::Right)
                    },
                    VK_UP => {
                        switch::windowgeometry::get_adjacent_window(
                            current,
                            switch::windowgeometry::Direction::Up)
                    },
                    VK_DOWN => {
                        switch::windowgeometry::get_adjacent_window(
                            current,
                            switch::windowgeometry::Direction::Down)
                    },
                    VK_TAB => {
                        switch::windowgeometry::get_next_overlapped_window(current)
                    },
                    _ => {
                        Err(anyhow::Error::from(Error::from(ERROR_INVALID_PARAMETER)))
                    }
                };

                if let Err(e) = adjacent_window {
                    switch::trace!("directional_switching", log::Level::Debug, "get_candidate_windows returned error: {:?}", e);
                    return;
                }

                let adjacent_window = adjacent_window.unwrap();
                let _ = set_foreground_window_terminal(adjacent_window);
                let timer = CreateThreadpoolTimer(Some(create_highlight_window), core::mem::transmute(adjacent_window), std::ptr::null());
                SetThreadpoolTimer(timer, &FILETIME::default(), 0, 0);
                // We have to do SetWindowPos last for VK_TAB otherwise set_foreground_window_terminal doesn't work.
                if vk == VK_TAB {
                    SetWindowPos(current, HWND_BOTTOM, 0, 0, 0, 0, 
                        SWP_NOMOVE | SWP_NOSIZE | SWP_DEFERERASE | SWP_NOACTIVATE | SWP_NOREDRAW);
                }
            });
        }
        switch::trace!("hotkey", log::Level::Info, "Caps lock modifier actions handled");
        return LRESULT(1);
    }
    switch::trace!("hotkey", log::Level::Info, "Exiting with CallNextHookEx");
    return CallNextHookEx(HOOK_HANDLE, code, wparam, lparam);
}

unsafe fn _kill_window_process(windowh: HWND) {
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

unsafe fn configure_quake_window(hwnd: HWND) -> Result<()> {
    if !hwnd.is_invalid() {
        // This can fail on windows 10 where the DWM options are not available.
        // TODO: check if options are supported.
        let _ = set_dwm_style(hwnd);
        ShowWindow(hwnd, SW_HIDE);
    }
    return Ok(());
}

fn initialize_index() {
    // let mut indexer_path = std::path::PathBuf::from(std::env::current_exe().unwrap().parent().unwrap());
    // indexer_path.push("indexer.exe");
    // let output = std::process::Command::new(indexer_path.as_os_str().to_str().unwrap().to_owned()).output().unwrap();
    let output = std::process::Command::new(switch::path::get_installed_exe_path("indexer.exe")).output().unwrap();
    println!("status: {}", output.status);
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();
    unsafe { let _ = switch::console::clear_console(); }
}

fn quake_terminal_runner(command: &str) -> anyhow::Result<()> {
    switch::log::initialize_log(log::Level::Debug, &["init", "runtime", "hotkey", "directional_switching"], switch::path::get_app_data_path("quake_terminal_runner.log")?)?;
    // log::info!("quake_terminal_runner started.");
    switch::trace!("init", log::Level::Info, "quake_terminal_runner started.");

    initialize_index();

    unsafe {
        // CoInitializeEx(0, COINIT_APARTMENTTHREADED).ok();

        // MAIN_THREAD_ID = GetCurrentThreadId();

        let mut waits = WaitList::new();
        let mut current_running_process = HANDLE(0);

        SetConsoleCtrlHandler(Some(ctrl_handler), BOOL(1));

        // TODO remove this event. And maybe other events.
        let run_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(RUN_QUAKE_EVENT_NAME)
        );
        assert!(waits.add(run_quake_event));

        let should_exit_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(EXIT_QUAKE_EVENT_NAME)
        );
        assert!(waits.add(should_exit_event));
        assert!(!should_exit_event.is_invalid());

        let open_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(OPEN_QUAKE_EVENT_NAME)
        );
        assert!(waits.add(open_quake_event));

        let hide_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            std::ffi::OsString::from(HIDE_QUAKE_EVENT_NAME)
        );
        assert!(waits.add(hide_quake_event));

        let btm_event = CreateEventW(
            std::ptr::null(),
            BOOL(0),
            BOOL(0),
            std::ffi::OsString::from(BTM_EVENT_NAME)
        );
        assert!(waits.add(btm_event));

        // Starting waiting for "start switch" messages.
        // let mut start_switch_read = HANDLE(0);
        let mut buf = [0u8; 256];
        let mut start_switch_read_overlapped = windows::Win32::System::IO::OVERLAPPED::default();
        start_switch_read_overlapped.hEvent = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            PCWSTR(std::ptr::null()),
        );
        let mut sa = windows::Win32::Security::SECURITY_ATTRIBUTES::default();
        sa.nLength = std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as u32;
        // owner builtin admin, group system, admin access full, everyone, deny full
        // Actually don't need this, mandatory integrity control will prevent
        // less than high integrity processes from accessing pipe.
        let dacl = "O:BAG:SYD:(A;OICI;GA;;;BA)(D;;FA;;;WD)\0";
        ConvertStringSecurityDescriptorToSecurityDescriptorA(
            PCSTR(dacl.as_ptr()),
            SDDL_REVISION_1,
            &mut (sa.lpSecurityDescriptor as *mut SECURITY_DESCRIPTOR),
            std::ptr::null_mut());
        let start_switch_read = CreateNamedPipeA(
            PCSTR("\\\\.\\Pipe\\QuakeTerminalRunner\0".as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_MESSAGE,
            1,
            4096,
            4096,
            0,
            &sa);

        // If testing pipe security, wait for cargo test --package switch --test pipesecurity -- openpipe --nocapture
        // because there can only be one writer.
        #[cfg(test_pipe_security)]
        Sleep(100000);

        START_SWITCH_WRITE = CreateFileA(
            PCSTR("\\\\.\\Pipe\\QuakeTerminalRunner\0".as_ptr()),
            FILE_GENERIC_WRITE,
            FILE_SHARE_NONE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE(0));

        LocalFree(sa.lpSecurityDescriptor as isize);

        // Can't use unamed pipe, doesn't support overlapped IO.
        // CreatePipe(&mut start_switch_read, &mut START_SWITCH_WRITE, std::ptr::null(), 0);
        // let mode = PIPE_NOWAIT;
        // SetNamedPipeHandleState(
        //     start_switch_read,
        //     &mode,
        //     std::ptr::null(),
        //     std::ptr::null());

        let _res = ReadFile(start_switch_read, 
            buf.as_mut_ptr() as _, 
            buf.len() as u32,
            std::ptr::null_mut(),
            &mut start_switch_read_overlapped);
        // switch::trace!("init", log::Level::Info, "ReadFile gle {} ret {}", GetLastError().0, res.0);
        assert!(GetLastError() == ERROR_IO_PENDING);
        SetLastError(NO_ERROR);
        assert!(waits.add(start_switch_read_overlapped.hEvent));

        let terminal_pid = windowprovider::getppid(GetCurrentProcessId());
        let quake_window = get_process_window(terminal_pid)?;
        configure_quake_window(quake_window)?;

        // backtick
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        if !RegisterHotKey(HWND(0), QUAKE_HOT_KEY_ID, MOD_ALT | MOD_NOREPEAT, VK_OEM_3.0 as u32).as_bool() {
            switch::trace!("hotkey", log::Level::Info, "RegisterHotKey returned {}", GetLastError().0);
        }

        // This hotkey is reserved.
        // if !RegisterHotKey(HWND(0), TERMINAL_HOT_KEY_ID, MOD_WIN | MOD_NOREPEAT, VK_RETURN.0 as u32).as_bool() {
        //     switch::trace!("hotkey", log::Level::Info, "RegisterHotKey returned {}", GetLastError().0);
        // }

        HOOK_HANDLE = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), HINSTANCE(0), 0);

        loop {
            let mut msg: MSG = std::mem::zeroed();

            match waits.wait() {
                WaitResult::Handle(h) => {
                    if h == run_quake_event {
                        // not using this anymore.
                        // continue;
                        // switch::console::clear_console()?;

                        let pid = switch::create_process::create_process(command.into());

                        let pid = if pid.is_err() {
                            set_event_by_name(HIDE_QUAKE_EVENT_NAME);
                            ResetEvent(run_quake_event);
                            continue
                        } else {
                            pid.unwrap()
                        };

                        current_running_process = OpenProcess(PROCESS_SYNCHRONIZE, BOOL(0), pid);
                        waits.add(current_running_process);
                        ResetEvent(run_quake_event);
                        set_foreground_window_terminal(quake_window)?;
                    } else if h == should_exit_event {
                        switch::trace!("init", log::Level::Info, "quake_terminal_runner exiting.");
                        break;
                    } else if h == open_quake_event {
                        switch::trace!("message_queue", log::Level::Info, "WaitForMultipleObjects: OPEN_WAIT");

                        if !IsWindowVisible(quake_window).as_bool() {
                            SetEvent(run_quake_event);
                            ShowWindow(quake_window, SW_SHOW);
                        }

                        // windows2::set_foreground_window_ex(quake_window);

                        let cmdline = "wt -w _quake fp --target 0".to_string();
                        switch::create_process::create_process(cmdline)?;

                        ResetEvent(open_quake_event);
                    } else if h == hide_quake_event {
                        switch::trace!("message_queue", log::Level::Info, "WaitForMultipleObjects: HIDE_WAIT");
                        ShowWindow(quake_window, SW_HIDE);
                        ShowWindow(quake_window, SW_MINIMIZE);
                        ResetEvent(hide_quake_event);
                    } else if h == current_running_process {
                        waits.remove(current_running_process);
                        current_running_process = HANDLE(0);
                        set_event_by_name(HIDE_QUAKE_EVENT_NAME);
                    } else if h == start_switch_read_overlapped.hEvent {
                        if current_running_process.is_invalid() {
                            let mut buf_read = 0u32;

                            GetOverlappedResult(start_switch_read, &start_switch_read_overlapped, &mut buf_read, BOOL(0));
                            let cmdline = format!("{} {}", &command, std::str::from_utf8(&buf[0..buf_read as usize]).unwrap());
                            let pid = switch::create_process::create_process(cmdline.clone());

                            if pid.is_err() {
                                //set_event_by_name(HIDE_QUAKE_EVENT_NAME);
                                // ResetEvent(run_quake_event);
                                //continue
                                switch::trace!("runtime", log::Level::Error, "create_process {} failed: {:?}", &cmdline, pid.err());
                            } else {
                                current_running_process = OpenProcess(PROCESS_SYNCHRONIZE, BOOL(0), pid.unwrap());
                                waits.add(current_running_process);
                            };
                        }

                        if !current_running_process.is_invalid() {
                            set_foreground_window_terminal(quake_window)?;
                        }

                        // Read for the next command.
                        ReadFile(start_switch_read, 
                            buf.as_mut_ptr() as _, 
                            buf.len() as u32,
                            std::ptr::null_mut(),
                            &mut start_switch_read_overlapped);
                        if GetLastError() != ERROR_IO_PENDING {
			                switch::trace!("runtime", log::Level::Info, "ReadFile start_switch_read failed with {}", GetLastError().0);
                        }
                        SetLastError(NO_ERROR);
                    } else if h == btm_event {
                        // Same as above but we want to run unelevated because the path for btm is medium integrity.
                        if current_running_process.is_invalid() {
                            let cmdline: String;
                            // cargo install bottom
                            let btm_path = "%USERPROFILE%\\.cargo\\bin\\btm.exe -b\0";
                            let mut expanded: [u8; 512] = [0; 512];
                            // PSTR(expanded.as_mut_ptr())
                            let len = windows::Win32::System::Environment::ExpandEnvironmentStringsA(PCSTR(btm_path.as_ptr()), &mut expanded) as usize;
                            // Exclude null terminator which is needed for ExpandEnvironmentStringsA but not for rust strings.
                            cmdline = String::from_utf8_lossy(&expanded[..len-1]).into();

                            let pid = switch::create_process::create_process(cmdline.clone());

                            if pid.is_err() {
                                //set_event_by_name(HIDE_QUAKE_EVENT_NAME);
                                // ResetEvent(run_quake_event);
                                //continue
                                switch::trace!("runtime", log::Level::Error, "create_medium_process {} failed: {:?}", &cmdline, pid.err());
                                continue;
                            } else {
                                current_running_process = OpenProcess(PROCESS_SYNCHRONIZE, BOOL(0), pid.unwrap());
                                waits.add(current_running_process);
                            };
                        }
                        if !current_running_process.is_invalid() {
                            set_foreground_window_terminal(quake_window)?;
                        }

                    } else {
                        assert!(false, "Unexpected MsgWaitForMultipleObjects signalled handle: {}.", h.0);
                    }
                },
                WaitResult::Message => {
                    while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                        match msg.message {
                            WM_HOTKEY => {
                                // println!("Hotkey pressed!");
                                switch::trace!("hotkey", log::Level::Info, "Hotkey pressed!");

                                if msg.wParam.0 as i32 == QUAKE_HOT_KEY_ID{
                                    let arg = "--mode window\0";
                                    let mut written = 0u32;
                                    WriteFile(
                                        START_SWITCH_WRITE,
                                        arg.as_bytes().as_ptr() as _,
                                        arg.len() as u32,
                                        &mut written,
                                        std::ptr::null_mut());
                                    assert!(written as usize == arg.len());
                                }

                                // if current_running_process.is_invalid() {
                                //     SetEvent(run_quake_event);
                                // } else {
                                //     set_foreground_window_terminal(quake_window)?;
                                // }
                            },
                            // WM_START_SWITCH => {
                            //     panic!("LOL do I really run commands received from window messages");
                            // }
                            _ => {
                                TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }
                        }
                    }
                },
                WaitResult::Error(err) => {
                    assert!(false, "Unexpected MsgWaitForMultipleObjects error: {}.", err);
                }
            }
        }

        UnhookWindowsHookEx(HOOK_HANDLE);
        UnregisterHotKey(HWND(0), QUAKE_HOT_KEY_ID);
        CloseHandle(start_switch_read_overlapped.hEvent);
        CloseHandle(should_exit_event);
        CloseHandle(run_quake_event);
        CloseHandle(open_quake_event);
        CloseHandle(hide_quake_event);

        DestroyWindow(quake_window); // Doesn't work..
        SendMessageW(quake_window, WM_CLOSE, WPARAM(0), LPARAM(0));
        SendMessageW(quake_window, WM_QUIT, WPARAM(0), LPARAM(0));
        // kill_window_process(quake_window);

        return Ok(());
    }
}

fn main() -> anyhow::Result<()> {
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
        println!("Stopping quakerun...");
        set_event_by_name(EXIT_QUAKE_EVENT_NAME);
        return Ok(());
    }

    // if matches.value_of("command").is_none() {
    //     println!("Need --command to be specified.");
    //     return Err(anyhow::Error::from(Error::from(ERROR_INVALID_PARAMETER)));
    // }

    if matches.occurrences_of("runner") == 1 && !matches.value_of("command").is_none() {
        println!("Quakerun starting as terminal runner.");
        return quake_terminal_runner(matches.value_of("command").unwrap());
    }

    let switch_default_path = switch::path::get_installed_exe_path("switch.exe");
    let command = matches.value_of("command").unwrap_or(&switch_default_path);

    unsafe {
        SetLastError(NO_ERROR);

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

        // Prevent this instance of quake terminal from registering default quake terminal hotkey.
        if !RegisterHotKey(HWND(0), QUAKE_WIN_HOT_KEY_ID, MOD_WIN | MOD_NOREPEAT, VK_OEM_3.0 as u32).as_bool() {
            println!("RegisterHotKey returned {}", GetLastError().0);
        }

        let quake_window = create_initial_quake_window(command)?;
        println!("Found quake window hwnd {:?}", quake_window);
        UnregisterHotKey(HWND(0), QUAKE_WIN_HOT_KEY_ID);

        CloseHandle(should_exit_event);

        Ok(())
    }
}