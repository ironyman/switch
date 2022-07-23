#![windows_subsystem = "windows"]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
};

use switch::log::*;

// Start a console subsystem program with no console.
fn main() -> Result<()> {
    // why does uncommenting this make it work?? cap p, cmd.exe
    switch::log::initialize_log(log::Level::Debug, &["init", "start"], switch::path::get_app_data_path("switch.log").unwrap()).unwrap();

    unsafe {
        windows::Win32::System::Console::AttachConsole(windows::Win32::System::Console::ATTACH_PARENT_PROCESS);
    }

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

    println!("\ncmdline: {}", cmdline);
    switch::trace!("start", log::Level::Info, "noconsole: {}", &cmdline);
    // unsafe { windows::Win32::System::Diagnostics::Debug::DebugBreak(); }

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
            let cmdline = (cmdline + "\0").encode_utf16().collect::<Vec<u16>>();
            windows::Win32::UI::Shell::ShellExecuteW(
                HWND(0),
                PCWSTR(std::ptr::null()),
                PCWSTR(cmdline.as_ptr()),
                PCWSTR(std::ptr::null()),
                PCWSTR(std::ptr::null()),
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0 as i32
            );       
            // let result = windows::Win32::UI::Shell::ShellExecuteA(
            //     HWND(0),
            //     PCSTR(std::ptr::null()),
            //     PCSTR(cmdline.as_ptr()),
            //     PCSTR(std::ptr::null()),
            //     PCSTR(std::ptr::null()),
            //     windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0 as i32
            // );
            // let msg = format!("return {}\n error: {:?}\n cmdline: {}\n exists: {}\0", 
            //     result.0,
            //     Error::from_win32(),
            //     cmdline,
            //     std::path::PathBuf::from(&cmdline).exists());
            // windows::Win32::UI::WindowsAndMessaging::MessageBoxA(HWND(0), PCSTR(msg.as_ptr()), PCSTR("lol\0".as_ptr()), windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE(0));
            
            return Ok(());
        }
    }
}