use std::path::Path;
use std::any::Any;

use windows::{
    Win32::Foundation::*,
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
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::{
    setforegroundwindow::set_foreground_window_terminal,
    listcontentprovider::ListItem
};
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

impl ListItem for WindowInfo {
    fn as_any(&self) -> &dyn Any {
        return self;
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        return self;
    }

    fn as_string(&self) -> String {
        let w = self.as_any().downcast_ref::<WindowInfo>().unwrap();
        return format!("{}", w);
    }

    fn as_matchable_string(&self) -> String {
        let w = self.as_any().downcast_ref::<WindowInfo>().unwrap();
        return w.window_text.clone();
    }
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
        // EnumWindows can fail with GLE E_HANDLE, maybe something in enum_window_proc is failing.
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
    query: String,
    terminal_host_pid: u32,
}

impl WindowProvider {
    pub fn new() -> Box<Self> {
        let quakerun_pid = getppid(unsafe { GetCurrentProcessId() });
        let terminal_host_pid = getppid(quakerun_pid);
        Box::new(WindowProvider {
            windows: enum_window().unwrap(),
            query: "".into(),
            terminal_host_pid,
        })
    }
}

impl ListContentProvider for WindowProvider {
    // type ListItem = WindowInfo;

    // fn query_for_items(&self) -> Vec<&WindowInfo> {
    fn query_for_items(&mut self) -> Vec<&mut dyn ListItem> {
        if self.windows.len() <= 1 {
            return vec![]
        }

        self.windows.iter_mut().filter(|w| {
            if w.process_id == self.terminal_host_pid {
                return false;
            }

            if w.image_name.to_lowercase().contains(&self.query.to_lowercase()) {
                return true;
            }
            if w.window_text.to_lowercase().contains(&self.query.to_lowercase()) {
                return true;
            }
            return false;
        }).map(|w| {
            w as &mut dyn ListItem
        }).collect()
    }

    fn query_for_names(&mut self) -> Vec<String> {
        self.query_for_items().iter().map(|w| {
            (*w).as_any().downcast_ref::<WindowInfo>().expect("This should work").to_string()
        }).collect::<Vec<String>>()
    }

    fn set_query(&mut self, query: String) {
        self.query = query;
    }

    fn start(&mut self, filtered_index: usize, _elevated: bool) {
        let windows = self.query_for_items();
        if filtered_index >= windows.len() {
            return;
        }
        crate::trace!("start", log::Level::Info, "Activate window: {}", windows[filtered_index].as_any().downcast_ref::<WindowInfo>().unwrap());
        set_foreground_window_terminal(windows[filtered_index].as_any().downcast_ref::<WindowInfo>().unwrap().windowh).unwrap();
    }

    fn remove(&mut self, filtered_index: usize) {
        let windows = self.query_for_items();
        if filtered_index >= windows.len() {
            return;
        }

        unsafe {
            // windows::Win32::UI::WindowsAndMessaging::CloseWindow(windows[filtered_index].windowh);
            SendMessageW(windows[filtered_index].as_any().downcast_ref::<WindowInfo>().unwrap().windowh, WM_CLOSE, WPARAM(0), LPARAM(0));

        }
        self.windows =  enum_window().unwrap();
    }
}
