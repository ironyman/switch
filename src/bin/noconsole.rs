#![windows_subsystem = "windows"]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
};

// Start a console subsystem program with no console.
fn main() -> Result<()> {

    if std::env::args().len() < 2 {
        return Ok(());
    }

    let mut method = std::env::args().nth(1).unwrap();
    let cmdline = if method.starts_with("--") {
        std::env::args().skip(2).collect::<Vec<String>>().join(" ")
    } else {
        method = "--createprocess".to_owned();
        std::env::args().skip(1).collect::<Vec<String>>().join(" ")
    };

    if cmdline.len() == 0 {
        return Ok(());
    }

    println!("cmdline: {}", cmdline);

    unsafe {
        if method == "--createprocess" {
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
                CREATE_NO_WINDOW | DETACHED_PROCESS,
                std::ptr::null(),
                PCWSTR(std::ptr::null()),
                &si,
                &mut pi
            );
            if !created.as_bool() {
                println!("Failed to start process with error {}", Error::from_win32());
                return Err(Error::from_win32());
            }
            println!("Started");

            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);

            return Ok(());
        } else {
            windows::Win32::UI::Shell::ShellExecuteA(
                HWND(0),
                PCSTR(std::ptr::null()),
                PCSTR(cmdline.as_ptr()),
                PCSTR(std::ptr::null()),
                PCSTR(std::ptr::null()),
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0 as i32
            );
            return Ok(());
        }
    }
}