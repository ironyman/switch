#![windows_subsystem = "windows"]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Threading::*,
};

// Start a console subsystem program with no console.
fn main() -> Result<()> {
    let cmdline = std::env::args().skip(1).collect::<Vec<String>>().join(" ");

    println!("cmdline: {}", cmdline);

    unsafe {
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
            CREATE_NO_WINDOW,
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
    }
}