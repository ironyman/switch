use std::path::Path;

use windows::{
    Win32::Foundation::{
        HWND,
        LPARAM,
        BOOL,
        CloseHandle,
        INVALID_HANDLE_VALUE,
    },
    Win32::UI::WindowsAndMessaging::{
        WINDOW_EX_STYLE,
        WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
        GWL_EXSTYLE,
        EnumWindows,
        GetWindowTextW,
        IsWindowVisible,
        GetWindowThreadProcessId,
        GetWindowLongW,
        WINDOW_STYLE,
        WS_POPUP,
        WS_CHILD,
        GWL_STYLE,
    },
    Win32::System::{ProcessStatus::K32GetProcessImageFileNameW, Threading::GetCurrentProcessId},
    Win32::System::Threading::{
        OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
    Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot,
        Process32First,
        Process32Next,
        TH32CS_SNAPPROCESS,
        PROCESSENTRY32
    },
};
use crate::setforegroundwindow::set_foreground_window_terminal;
use crate::listcontentprovider::ListContentProvider;

use crate::log::*;

//use windows::Win32::UI::WindowsAndMessaging::*;

pub struct WindowInfo {
    pub windowh: HWND,
    pub window_text: String,
    pub process_id: u32,
    pub image_name: String,
    pub ex_style: WINDOW_EX_STYLE,
    pub style: WINDOW_STYLE,
}

impl Drop for WindowInfo {
    fn drop(&mut self) {

    }
}

impl std::fmt::Display for WindowInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // write!(f, "{}: {} ({:#x}, {:#x}, {:#x})",
        //     self.image_name, self.window_text,
        //     self.process_id, self.style.0, self.ex_style.0)
        write!(f, "{}: {} ({})",
            self.image_name, self.window_text,
            self.process_id)
    }
}

// https://github.com/microsoft/windows-rs/blob/master/crates/samples/enum_windows/Cargo.toml
extern "system" fn enum_window_proc(windowh: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Foundation/struct.BOOL.html#impl-Into%3CU%3E
        if !IsWindowVisible(windowh).as_bool() {
            return true.into()
        }

        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/WindowsAndMessaging/struct.WINDOW_EX_STYLE.html
        // what a mess, you have to access the pub struct member...
        // https://docs.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles
        let ex_style = WINDOW_EX_STYLE(GetWindowLongW(windowh, GWL_EXSTYLE) as u32);
        if (ex_style & WS_EX_NOACTIVATE).0 != 0 ||
           (ex_style & WS_EX_TOOLWINDOW).0 != 0 {
            // WS_EX_NOREDIRECTIONBITMAP is for windows that use DirectComposition for rendering.
            // E.g. Sciter creates windows with WS_EX_NOREDIRECTIONBITMAP to support Acrylic composition effects like here:
        //    (ex_style & WS_EX_NOREDIRECTIONBITMAP).0 != 0 {
            return true.into()
        }

        let style = WINDOW_STYLE(GetWindowLongW(windowh, GWL_STYLE) as u32);
        if (style & WS_POPUP).0 != 0 ||
           (style & WS_CHILD).0 != 0 {
               return true.into()
           }
        let mut window_text: [u16; 512] = [0; 512];
        let len = GetWindowTextW(windowh, &mut window_text);
        let window_text = String::from_utf16_lossy(&window_text[..len as usize]);

        if window_text.len() == 0 {
            return true.into()
        }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(windowh, &mut process_id);

        let processh = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL(0), process_id);

        let mut image_name: [u16; 512] = [0; 512];
        let len = K32GetProcessImageFileNameW(processh, &mut image_name);
        CloseHandle(processh);
        if len == 0{
            return false.into();
        }
        let image_name = String::from_utf16_lossy(&image_name[..len as usize]);
        let image_name: String = Path::new(&image_name).file_stem().unwrap().to_str().unwrap().into();

        let windows = lparam.0 as *mut Vec<WindowInfo>;

        (*windows).push(WindowInfo { windowh, window_text, process_id, image_name, style, ex_style });

        // if !text.is_empty() {
        //     println!("title: {}\npid: {}\nlong: {}\n", text, process_id, window_long.0);
        // }

        true.into()
    }
}

// fn enum_window() -> windows::core::Result<()> {
pub fn enum_window() -> windows::core::Result<Vec<WindowInfo>> {
    let mut windows = Vec::<WindowInfo>::new();

    unsafe { 
//        EnumWindows(Some(enum_window_proc), LPARAM(&mut windows as *mut _ as isize)).ok()
        EnumWindows(Some(enum_window_proc), LPARAM(&mut windows as *mut _ as isize)).ok()?;
        return Ok(windows);
    }
}

pub fn getppid(pid: u32) -> u32 {
    unsafe {
        let mut pe32: PROCESSENTRY32 = std::mem::zeroed();

        let mut ppid: u32 = u32::MAX;

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
            if pe32.th32ProcessID == pid {
                ppid = pe32.th32ParentProcessID;
                break;
            }
            if !Process32Next(snapshot, &mut pe32).as_bool() {
                break;
            }
        }

        CloseHandle(snapshot);

        return ppid;
    }
}

pub struct WindowProvider {
    windows: Vec<WindowInfo>,
    filter: String,
    terminal_host_pid: u32,
}

impl WindowProvider {
    pub fn new() -> Box<Self> {
        let quakerun_pid = getppid(unsafe { GetCurrentProcessId() });
        let terminal_host_pid = getppid(quakerun_pid);
        Box::new(WindowProvider {
            windows: enum_window().unwrap(),
            filter: "".into(),
            terminal_host_pid,
        })
    }

    pub fn get_filtered_window_list(&self) -> Vec<&WindowInfo> {
        if self.windows.len() <= 1 {
            return vec![]
        }

        self.windows.iter().filter(|&w| {
            if w.process_id == self.terminal_host_pid {
                return false;
            }

            if w.image_name.to_lowercase().contains(&self.filter) {
                return true;
            }
            if w.window_text.to_lowercase().contains(&self.filter) {
                return true;
            }
            return false;
        }).collect()
    }
}

impl ListContentProvider for WindowProvider {
    fn get_filtered_list(&self) -> Vec<String> {
        self.get_filtered_window_list().iter().map(|&w| {
            w.to_string()
        }).collect::<Vec<String>>()
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    fn activate(&self, filtered_index: usize) {
        let windows = self.get_filtered_window_list();
        if filtered_index >= windows.len() {
            return;
        }
        crate::trace!("activate", log::Level::Info, "Activate window: {}", windows[filtered_index]);
        set_foreground_window_terminal(windows[filtered_index].windowh).unwrap();
    }
}
