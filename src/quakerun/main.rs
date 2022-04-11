// cargo run --bin quakerun C:\Users\changyl\Desktop\switch\target\debug\switch.exe
// #![cfg_attr(my_feature_name_i_made_up, windows_subsystem = "windows")]

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

#[path="../windows2.rs"]
mod windows2;

const WAIT_QUAKE_SECONDS: u32 = 60;
const QUAKE_HOT_KEY_ID: i32 = 1;

unsafe fn create_process(cmdline: String) -> Result<u32> {
    let mut cmdline = cmdline.encode_utf16().collect::<Vec<u16>>();
    let mut si: STARTUPINFOW = std::mem::zeroed();
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let created = CreateProcessW(
        PCWSTR(std::ptr::null()),
        PWSTR(cmdline.as_mut_ptr()  as *mut _),
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

unsafe fn create_initial_quake_window() -> Result<HWND> {
    let cmdline = "wt -w _quake cmd /c echo I am waiting for commands & timeout /t -1 /nobreak\0".to_string();
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

        let hwnd = get_process_window(windows_terminal_pid).unwrap();

        if !hwnd.is_invalid() {
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

    return Ok(());
}

unsafe extern "system" fn ctrl_handler(ctrltype: u32) -> BOOL {
    if ctrltype == CTRL_C_EVENT {
        println!("Ctrl-c hit, exiting");
        //DebugBreak();
        PostQuitMessage(0);
    }
    return BOOL(1);
}

fn main() -> Result<()> {
    unsafe {
        // https://github.com/rust-lang/rust/issues/7235#:~:text=Every%20string%20allocation%20in%20rust%20is%20null%20terminated,that%20are%20considered%20part%20of%20the%20utf-8%20data.
        // strings are not null terminated?
        // Local events are in \Sessions\1\BaseNamedObjects\
        // str escape codes /u{7f}, \x41  has to be ascii
        // https://github.com/rust-lang/rust/issues/18415 
        let open_quake_event_name: Vec<u16> = "OpenQuake\0".encode_utf16().collect::<Vec<u16>>();
        let hide_quake_event_name: Vec<u16> = "HideQuake\0".encode_utf16().collect::<Vec<u16>>();

        SetLastError(NO_ERROR);

        let mut open_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            PCWSTR(open_quake_event_name.as_ptr() as *const _)
        );

        assert!(!open_quake_event.is_invalid());

        if Error::from_win32().code() == ERROR_ALREADY_EXISTS.to_hresult() {
            let option = std::env::args().skip(1).next();
            match option.unwrap_or("".into()).as_str() {
                "--open" => {
                    open_quake_event = OpenEventW(THREAD_SYNCHRONIZE.0 | EVENT_MODIFY_STATE , BOOL(0), PCWSTR(open_quake_event_name.as_ptr() as *const _));
                    assert!(!open_quake_event.is_invalid());
                    SetEvent(open_quake_event);
                    CloseHandle(open_quake_event);
                },
                "--hide" => {
                    let hide_quake_event = OpenEventW(THREAD_SYNCHRONIZE.0 | EVENT_MODIFY_STATE, BOOL(0), PCWSTR(hide_quake_event_name.as_ptr() as *const _));
                    assert!(!hide_quake_event.is_invalid());
                    SetEvent(hide_quake_event);
                    CloseHandle(hide_quake_event);
                },
                _ => {
                }
            };
            
            return Ok(());
        }

        let hide_quake_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            PCWSTR(hide_quake_event_name.as_ptr() as *const _)
        );

        let should_exit_event = CreateEventW(
            std::ptr::null(),
            BOOL(1),
            BOOL(0),
            PCWSTR(std::ptr::null())
        );

        let quake_window = create_initial_quake_window()?;
        
        println!("Found quake window hwnd {:?}", quake_window);

        set_dwm_style(quake_window)?;
        ShowWindow(quake_window, SW_HIDE);

        // backtick
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        RegisterHotKey(HWND(0), QUAKE_HOT_KEY_ID, MOD_ALT | MOD_NOREPEAT, VK_OEM_3.0 as u32);

        SetConsoleCtrlHandler(Some(ctrl_handler), BOOL(1));

        let quake_server_thread = std::thread::spawn(move || {
            loop {
                // match WaitForSingleObject(open_quake_event, INFINITE) {
                //     WAIT_OBJECT_0 => {
                //         let args: Vec<String> = std::env::args().skip(1).collect();
                //         create_process("wt -w _quake ".to_string() + &args.join(" ")).unwrap();
                //         ResetEvent(open_quake_event);
                //     },
                //     _ => {
                //         assert!(false, "Unexpected WaitForSingleObject failure");
                //     }
                // }
                let events = [open_quake_event, hide_quake_event, should_exit_event];
                const OPEN_WAIT: u32 = WAIT_OBJECT_0;
                const HIDE_WAIT: u32 = WAIT_OBJECT_0 + 1;
                const EXIT_WAIT: u32 = WAIT_OBJECT_0 + 2;

                match WaitForMultipleObjects(&events, BOOL(0), INFINITE) {
                    OPEN_WAIT => {
                        println!("WaitForMultipleObjects: OPEN_WAIT");

                        if !IsWindowVisible(quake_window).as_bool() {
                            let args: Vec<String> = std::env::args().skip(1).collect();
                            let cmdline = "wt -w _quake nt cmd /c ".to_string() 
                                + &args.join(" ")
                                + &format!(" & {} --hide\0", std::env::current_exe().unwrap().to_str().unwrap());
                            println!("Running {}", cmdline);
                            create_process(cmdline).unwrap();
                            ShowWindow(quake_window, SW_SHOW);
                        } else {
                            windows2::set_foreground_window_ex(quake_window);
                        }
                        ResetEvent(open_quake_event);
                    },
                    HIDE_WAIT => {
                        println!("WaitForMultipleObjects: HIDE_WAIT");
                        ShowWindow(quake_window, SW_HIDE);
                        ResetEvent(hide_quake_event);
                    },
                    EXIT_WAIT => {
                        break;
                    }
                    _ => {
                        assert!(false, "Unexpected WaitForMultipleObjects failure");
                    }
                }
            }
        });

        let mut msg: MSG = std::mem::zeroed();

        while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            println!("Got message {}", msg.message);
            match msg.message {
                WM_HOTKEY => {
                    // println!("Hotkey pressed!");
                    SetEvent(open_quake_event);
                },
                _ => {}
            }
        }

        UnregisterHotKey(HWND(0), QUAKE_HOT_KEY_ID);
        
        SetEvent(hide_quake_event);
        quake_server_thread.join().expect("The thread being joined has panicked");

        CloseHandle(hide_quake_event);
        CloseHandle(open_quake_event);
        DestroyWindow(quake_window);

        Ok(())
    }
}