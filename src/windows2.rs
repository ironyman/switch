use std::path::Path;

use windows::{
    core::{
        PCSTR
    },
    Win32::Foundation::{
        HWND,
        LPARAM,
        WPARAM,
        BOOL,
        LRESULT,
        HINSTANCE,
        CloseHandle,
        INVALID_HANDLE_VALUE,
        HANDLE
    },
    Win32::UI::WindowsAndMessaging::{
        WINDOW_EX_STYLE,
        WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
        WS_EX_NOREDIRECTIONBITMAP,
        GWL_EXSTYLE,
        EnumWindows,
        GetWindowTextW,
        IsWindowVisible,
        GetWindowThreadProcessId,
        GetWindowLongW,
        GetForegroundWindow,
        SetForegroundWindow,
        SetWindowPos,
        HWND_TOPMOST,
        HWND_NOTOPMOST,
        SWP_NOSIZE,
        SWP_NOMOVE,
        SWP_SHOWWINDOW,
        BringWindowToTop,
        PostQuitMessage,
        DefWindowProcA,
        WM_DESTROY,
        WM_PAINT,
        CreateWindowExA,
        RegisterClassA,
        LoadCursorW,
        GetMessageA,
        PeekMessageA,
        DispatchMessageA,
        MSG,
        WNDCLASSA,
        CS_HREDRAW,
        CS_VREDRAW,
        IDC_ARROW,
        WS_OVERLAPPEDWINDOW,
        WS_VISIBLE,
        CW_USEDEFAULT,
        PM_REMOVE,
        DestroyWindow,
        IsHungAppWindow,
        FindWindowA,
        IsIconic,
        ShowWindow,
        SW_RESTORE,
        WINDOW_STYLE,
        WS_POPUP,
        WS_CHILD,
        GWL_STYLE,
        GetWindow,
        GW_OWNER,
        SW_SHOW,
    },
    Win32::UI::Input::KeyboardAndMouse::{
        SetFocus,
        SetActiveWindow,
    },
    Win32::System::{ProcessStatus::K32GetProcessImageFileNameW, Threading::Sleep},
    Win32::System::LibraryLoader::{
        LoadLibraryA,
        GetProcAddress,
        FreeLibrary,
    },
    Win32::System::Threading::{
        OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_VM_WRITE,
        PROCESS_VM_READ,
        PROCESS_VM_OPERATION,
        PROCESS_CREATE_THREAD,
        PROCESS_QUERY_INFORMATION,
        AttachThreadInput,
        CreateRemoteThread,
        WaitForSingleObject,
        GetCurrentThreadId,
    },
    Win32::System::Memory::{
        VirtualAllocEx,
        PAGE_EXECUTE_READWRITE,
        MEM_COMMIT,
        VirtualFreeEx,
        MEM_RELEASE,
    },
    Win32::System::Diagnostics::Debug::{
        WriteProcessMemory,
    },
    Win32::Graphics::Gdi::ValidateRect,
    Win32::System::{LibraryLoader::GetModuleHandleA, Threading::GetCurrentProcessId}, 
    Win32::System::WindowsProgramming::{
        INFINITE,
    },
    Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot,
        Process32First,
        Process32Next,
        TH32CS_SNAPPROCESS,
        PROCESSENTRY32
    },
};

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
    static mut CONHOST_PID: u32 = 0;

    unsafe {
        if CONHOST_PID != 0 {
            CONHOST_PID = GetCurrentProcessId();
            loop {
                let parent = getppid(CONHOST_PID);
                if parent != u32::MAX {
                    CONHOST_PID = parent;
                } else {
                    break;
                }
            }
        }
        
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
        
        if process_id == CONHOST_PID {
            return true.into()
        }

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

// https://stackoverflow.com/questions/23715026/allow-background-application-to-set-foreground-window-of-different-process
// need a window to receive input
extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as u32 {
            WM_PAINT => {
                println!("WM_PAINT");
                ValidateRect(window, std::ptr::null());
                LRESULT(0)
            }
            WM_DESTROY => {
                println!("WM_DESTROY");
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}

fn create_window() -> windows::core::Result<HWND> {
    unsafe {
        let instance = GetModuleHandleA(None);
        debug_assert!(instance.0 != 0);

        let window_class = PCSTR(b"window\0".as_ptr());

        let wc = WNDCLASSA {
            hCursor: LoadCursorW(None, IDC_ARROW),
            hInstance: instance,
            lpszClassName: window_class,

            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        };

        let atom = RegisterClassA(&wc);
        debug_assert!(atom != 0);

        let window = CreateWindowExA(Default::default(), 
        window_class, 
           PCSTR(b"window title\0".as_ptr()), 
           WS_OVERLAPPEDWINDOW | WS_VISIBLE, 
            -1000, 
            -1, 
            1, 
            1, 
            None, 
            None, 
            instance, 
            std::ptr::null());
        Ok(window)
        // let mut message = MSG::default();

        // while GetMessageA(&mut message, HWND(0), 0, 0).into() {
        //     DispatchMessageA(&message);
        // }

        // Ok(())
    }
}

// https://stackoverflow.com/questions/39451928/put-application-to-foreground
// void SetForegroundWindowForce(HWND hWnd)
// {
//    HWND hWndForeground = ::GetForegroundWindow();
//    if(hWndForeground == hWnd) return;

//    DWORD Strange = ::GetWindowThreadProcessId(hWndForeground, NULL);
//    DWORD My = ::GetWindowThreadProcessId(hWnd, NULL);
//    if( !::AttachThreadInput(My, Strange, TRUE) )
//    {
//       ASSERT(0);
//    }
//    ::SetForegroundWindow(hWnd);
//    ::BringWindowToTop(hWnd);
//    if( !::AttachThreadInput(My, Strange, FALSE) )
//    {
//       ASSERT(0);
//    }
// }

fn set_foreground_window(windowh: HWND) -> windows::core::Result<()> {
    unsafe {
        let foreground = GetForegroundWindow();

        let foreground_thread_id = GetWindowThreadProcessId(foreground, std::ptr::null_mut() as *mut u32);
        let target_thread_id = GetWindowThreadProcessId(windowh, std::ptr::null_mut() as *mut _);
        AttachThreadInput(target_thread_id, foreground_thread_id, BOOL(1)).ok()?;

        SetForegroundWindow(windowh);
        BringWindowToTop(windowh);
        SetFocus(windowh);

        AttachThreadInput(target_thread_id, foreground_thread_id, BOOL(0)).ok()?;

        Ok(())
    }
}

// This doesn't work because LoadLibraryA refers to a stub this executable that jumps to LoadLibraryA in kernel32.dll
// to make this work we'd have to do getprocaddress and put the address of LoadLibraryA and GetProcAddress 
// in this function, maybe use assembly
// https://itfanr.gitbooks.io/rust-doc-en/content/inline-assembly.html
// too much work.
extern "system" fn set_foreground_window_remote(windowh: HWND) -> BOOL {
    unsafe {
        let instance = LoadLibraryA(PCSTR(b"user32.dll\0".as_ptr()));
        // let sfgw = core::mem::transmute::<
        //     unsafe extern "system" fn() -> isize,
        //     extern "system" fn(hwnd: HWND) -> BOOL
        // >(GetProcAddress(instance, PCSTR("SetForegroundWindow\0".as_ptr())).unwrap());

        // sfgw(windowh);
        //FreeLibrary(instance);
        BOOL(1)
    }
}

extern "system" fn set_foreground_window_remote_end() {}

fn set_foreground_window_in_foreground(windowh: HWND) -> windows::core::Result<()> {
    unsafe {
        let foreground = GetForegroundWindow();

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(foreground, &mut process_id);
        
        let processh = OpenProcess(
            PROCESS_VM_WRITE | PROCESS_VM_READ | PROCESS_VM_OPERATION | 
            PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION, BOOL(0), process_id);

        // Write param
        let remote_param = VirtualAllocEx(processh, std::ptr::null(),
             std::mem::size_of::<HWND>(), MEM_COMMIT, PAGE_EXECUTE_READWRITE);

        if remote_param.is_null() {
            return Err(windows::core::Error::from_win32())
        }

        let mut written: usize = 0;
        WriteProcessMemory(processh, remote_param, core::mem::transmute(&windowh), std::mem::size_of::<HWND>(), &mut written);

        if written != std::mem::size_of::<HWND>() {
            return Err(windows::core::Error::from_win32())
        }

        // Write proc
        let mut proc_size = set_foreground_window_remote_end as *const () as isize - set_foreground_window_remote as *const () as isize;
        if proc_size <= 0 || proc_size > 0x4096 {
            proc_size = 4096;
        }
        let proc_size = proc_size as usize;

        let remote_proc = VirtualAllocEx(processh, std::ptr::null(),
            proc_size, MEM_COMMIT, PAGE_EXECUTE_READWRITE);

        if remote_proc.is_null() {
            return Err(windows::core::Error::from_win32())
        }

        let mut written: usize = 0;
        WriteProcessMemory(processh, remote_proc, set_foreground_window_remote as *const core::ffi::c_void, proc_size, &mut written);

        if written != proc_size {
            return Err(windows::core::Error::from_win32())
        }

        let remote_proc = core::mem::transmute::<*mut core::ffi::c_void, unsafe extern "system" fn(lpthreadparameter: *mut ::core::ffi::c_void) -> u32>(remote_proc);

        let threadh = CreateRemoteThread(processh, std::ptr::null(), 0, Some(remote_proc),
            remote_param, 0, std::ptr::null_mut());
        if threadh.is_invalid() {
            return Err(windows::core::Error::from_win32())
        }

        WaitForSingleObject(threadh, INFINITE);

        VirtualFreeEx(processh, remote_proc as *mut core::ffi::c_void, 0, MEM_RELEASE);
        VirtualFreeEx(processh, remote_param, 0, MEM_RELEASE);
        CloseHandle(threadh);
        CloseHandle(processh);

        Ok(())
    }
}

// From ahk https://github.com/AutoHotkey/AutoHotkey/blob/master/Source/window.cpp
const SLEEP_INTERVAL: u32 = 10;

unsafe fn attempt_set_foreground(target_window: HWND, fore_window: HWND) -> HWND {
    // Returns NULL if target_window or its owned-window couldn't be brought to the foreground.
    // Otherwise, on success, it returns either target_window or an HWND owned by target_window.
	// Probably best not to trust its return value.  It's been shown to be unreliable at times.
	// Example: I've confirmed that SetForegroundWindow() sometimes (perhaps about 10% of the time)
	// indicates failure even though it succeeds.  So we specifically check to see if it worked,
	// which helps to avoid using the keystroke (2-alts) method, because that may disrupt the
	// desired state of the keys or disturb any menus that the user may have displayed.
	// Also: I think the 2-alts last-resort may fire when the system is lagging a bit
	// (i.e. a drive spinning up) and the window hasn't actually become active yet,
	// even though it will soon become active on its own.  Also, SetForegroundWindow() sometimes
	// indicates failure even though it succeeded, usually because the window didn't become
	// active immediately -- perhaps because the system was under load -- but did soon become
	// active on its own (after, say, 50ms or so).  UPDATE: If SetForegroundWindow() is called
	// on a hung window, at least when AttachThreadInput is in effect and that window has
	// a modal dialog (such as MSIE's find dialog), this call might never return, locking up
	// our thread.  So now we do this fast-check for whether the window is hung first (and
	// this call is indeed very fast: its worst case is at least 30x faster than the worst-case
	// performance of the ABORT-IF-HUNG method used with SendMessageTimeout.
	// UPDATE for v1.0.42.03: To avoid a very rare crashing issue, IsWindowHung() is no longer called
	// here, but instead by our caller.  Search on "v1.0.42.03" for more comments.
	let result = SetForegroundWindow(target_window);
	// Note: Increasing the sleep time below did not help with occurrences of "indicated success
	// even though it failed", at least with metapad.exe being activated while command prompt
	// and/or AutoIt2's InputBox were active or present on the screen:
	Sleep(SLEEP_INTERVAL); // Specify param so that it will try to specifically sleep that long.
	let new_fore_window = GetForegroundWindow();
	if new_fore_window == target_window {
		return target_window;
	}
	if new_fore_window != fore_window && target_window == GetWindow(new_fore_window, GW_OWNER) {
    // The window we're trying to get to the foreground is the owner of the new foreground window.
		// This is considered to be a success because a window that owns other windows can never be
		// made the foreground window, at least if the windows it owns are visible.
		return new_fore_window;
    }
	return HWND(0);
}



unsafe fn _set_foreground_window_ex(target_window: HWND) -> HWND {
    // Caller must have ensured that target_window is a valid window or NULL, since we
    // don't call IsWindow() here.
	if target_window.is_invalid() {
		return HWND(0)
    }

	// v1.0.42.03: Calling IsWindowHung() once here rather than potentially more than once in attempt_set_foreground()
	// solves a crash that is not fully understood, nor is it easily reproduced (it occurs only in release mode,
	// not debug mode).  It's likely a bug in the API's IsHungAppWindow(), but that is far from confirmed.
    let target_thread = GetWindowThreadProcessId(target_window, std::ptr::null_mut());

    let current_thread_id = GetCurrentThreadId();

	if target_thread != current_thread_id && IsHungAppWindow(target_window).as_bool() { 
        // Calls to IsWindowHung should probably be avoided if the window belongs to our thread.  Relies upon short-circuit boolean order.
		return HWND(0)
    }

	let mut orig_foreground_wnd = GetForegroundWindow();

	// AutoIt3: If there is not any foreground window, then input focus is on the TaskBar.
	// MY: It is definitely possible for GetForegroundWindow() to return NULL, even on XP.
	if orig_foreground_wnd.is_invalid() {
		orig_foreground_wnd = FindWindowA(PCSTR(b"Shell_TrayWnd\0".as_ptr()), PCSTR(std::ptr::null()));
    }

	if target_window == orig_foreground_wnd{
        // It's already the active window.
		return target_window;
    }

	if IsIconic(target_window).as_bool() {
		// This might never return if target_window is a hung window.  But it seems better
		// to do it this way than to use the PostMessage() method, which might not work
		// reliably with apps that don't handle such messages in a standard way.
		// A minimized window must be restored or else SetForegroundWindow() always(?)
		// won't work on it.  UPDATE: ShowWindowAsync() would prevent a hang, but
		// probably shouldn't use it because we rely on the fact that the message
		// has been acted on prior to trying to activate the window (and all Async()
		// does is post a message to its queue):
		ShowWindow(target_window, SW_RESTORE);
    }

	// This causes more trouble than it's worth.  In fact, the AutoIt author said that
	// he didn't think it even helped with the IE 5.5 related issue it was originally
	// intended for, so it seems a good idea to NOT to this, especially since I'm 80%
	// sure it messes up the Z-order in certain circumstances, causing an unexpected
	// window to pop to the foreground immediately after a modal dialog is dismissed:
	//BringWindowToTop(target_window); // AutoIt3: IE 5.5 related hack.

	let mut new_foreground_wnd = HWND(0);

    const ACTIVATE_FORCE: bool = true;
	if !ACTIVATE_FORCE {
	    // if (g_os.IsWin95() || (!g_os.IsWin9x() && !g_os.IsWin2000orLater())))  // Win95 or NT
		// Try a simple approach first for these two OS's, since they don't have
		// any restrictions on focus stealing:
        new_foreground_wnd = attempt_set_foreground(target_window, orig_foreground_wnd);
        if !new_foreground_wnd.is_invalid() {
			return new_foreground_wnd;
        }
		// Otherwise continue with the more drastic methods below.
    }

	// MY: The AttachThreadInput method, when used by itself, seems to always
	// work the first time on my XP system, seemingly regardless of whether the
	// "allow focus steal" change has been made via SystemParametersInfo()
	// (but it seems a good idea to keep the SystemParametersInfo() in effect
	// in case Win2k or Win98 needs it, or in case it really does help in rare cases).
	// In many cases, this avoids the two SetForegroundWindow() attempts that
	// would otherwise be needed; and those two attempts cause some windows
	// to flash in the taskbar, such as Metapad and Excel (less frequently) whenever
	// you quickly activate another window after activating it first (e.g. via hotkeys).
	// So for now, it seems best just to use this method by itself.  The
	// "two-alts" case never seems to fire on my system?  Maybe it will
	// on Win98 sometimes.
	// Note: In addition to the "taskbar button flashing" annoyance mentioned above
	// any SetForegroundWindow() attempt made prior to the one below will,
	// as a side-effect, sometimes trigger the need for the "two-alts" case
	// below.  So that's another reason to just keep it simple and do it this way
	// only.



	let mut is_attached_my_to_fore = BOOL(0);
    let mut is_attached_fore_to_target = BOOL(0);
	let mut fore_thread: u32 = 0;

    // Might be NULL from above.
	if !orig_foreground_wnd.is_invalid() {
		// Based on MSDN docs, these calls should always succeed due to the other
		// checks done above (e.g. that none of the HWND's are NULL):
		fore_thread = GetWindowThreadProcessId(orig_foreground_wnd, std::ptr::null_mut());

		// MY: Normally, it's suggested that you only need to attach the thread of the
		// foreground window to our thread.  However, I've confirmed that doing all three
		// attaches below makes the attempt much more likely to succeed.  In fact, it
		// almost always succeeds whereas the one-attach method hardly ever succeeds the first
		// time (resulting in a flashing taskbar button due to having to invoke a second attempt)
		// when one window is quickly activated after another was just activated.
		// AutoIt3: Attach all our input threads, will cause SetForeground to work under 98/Me.
		// MSDN docs: The AttachThreadInput function fails if either of the specified threads
		// does not have a message queue (My: ok here, since any window's thread MUST have a
		// message queue).  [It] also fails if a journal record hook is installed.  ... Note
		// that key state, which can be ascertained by calls to the GetKeyState or
		// GetKeyboardState function, is reset after a call to AttachThreadInput.  You cannot
		// attach a thread to a thread in another desktop.  A thread cannot attach to itself.
		// Therefore, idAttachTo cannot equal idAttach.  Update: It appears that of the three,
		// this first call does not offer any additional benefit, at least on XP, so not
		// using it for now:
		//if (g_MainThreadID != target_thread) // Don't attempt the call otherwise.
		//	AttachThreadInput(g_MainThreadID, target_thread, TRUE);
		if fore_thread != 0 && current_thread_id != fore_thread && !IsHungAppWindow(orig_foreground_wnd).as_bool() {
			is_attached_my_to_fore = AttachThreadInput(current_thread_id, fore_thread, BOOL(1));
        }
		if fore_thread != 0 && target_thread != 0 && fore_thread != target_thread { // IsWindowHung(target_window) was called earlier.
			is_attached_fore_to_target = AttachThreadInput(fore_thread, target_thread, BOOL(1));
        }
	}

	// The log showed that it never seemed to need more than two tries.  But there's
	// not much harm in trying a few extra times.  The number of tries needed might
	// vary depending on how fast the CPU is:
	for i in 0..5 {
        new_foreground_wnd = attempt_set_foreground(target_window, orig_foreground_wnd);
        if !new_foreground_wnd.is_invalid() {
			break;
		}
	}

	// I decided to avoid the quick minimize + restore method of activation.  It's
	// not that much more effective (if at all), and there are some significant
	// disadvantages:
	// - This call will often hang our thread if target_window is a hung window: ShowWindow(target_window, SW_MINIMIZE)
	// - Using SW_FORCEMINIMIZE instead of SW_MINIMIZE has at least one (and probably more)
	// side effect: When the window is restored, at least via SW_RESTORE, it is no longer
	// maximized even if it was before the minmize.  So don't use it.
	if new_foreground_wnd.is_invalid() { // Not successful yet.
		// Some apps may be intentionally blocking us by having called the API function
		// LockSetForegroundWindow(), for which MSDN says "The system automatically enables
		// calls to SetForegroundWindow if the user presses the ALT key or takes some action
		// that causes the system itself to change the foreground window (for example,
		// clicking a background window)."  Also, it's probably best to avoid doing
		// the 2-alts method except as a last resort, because I think it may mess up
		// the state of menus the user had displayed.  And of course if the foreground
		// app has special handling for alt-key events, it might get confused.
		// My original note: "The 2-alts case seems to mess up on rare occasions,
		// perhaps due to menu weirdness triggered by the alt key."
		// AutoIt3: OK, this is not funny - bring out the extreme measures (usually for 2000/XP).
		// Simulate two single ALT keystrokes.  UPDATE: This hardly ever succeeds.  Usually when
		// it fails, the foreground window is NULL (none).  I'm going to try an Win-tab instead,
		// which selects a task bar button.  This seems less invasive than doing an alt-tab
		// because not only doesn't it activate some other window first, it also doesn't appear
		// to change the Z-order, which is good because we don't want the alt-tab order
		// that the user sees to be affected by this.  UPDATE: Win-tab isn't doing it, so try
		// Alt-tab.  Alt-tab doesn't do it either.  The window itself (metapad.exe is the only
		// culprit window I've found so far) seems to resist being brought to the foreground,
		// but later, after the hotkey is released, it can be.  So perhaps this is being
		// caused by the fact that the user has keys held down (logically or physically?)
		// Releasing those keys with a key-up event might help, so try that sometime:
		
        // TODO: implement these?
        // KeyEvent(KEYDOWNANDUP, VK_MENU);
		// KeyEvent(KEYDOWNANDUP, VK_MENU);

		//KeyEvent(KEYDOWN, VK_LWIN);
		//KeyEvent(KEYDOWN, VK_TAB);
		//KeyEvent(KEYUP, VK_TAB);
		//KeyEvent(KEYUP, VK_LWIN);
		//KeyEvent(KEYDOWN, VK_MENU);
		//KeyEvent(KEYDOWN, VK_TAB);
		//KeyEvent(KEYUP, VK_TAB);
		//KeyEvent(KEYUP, VK_MENU);
		// Also replacing "2-alts" with "alt-tab" below, for now:

        new_foreground_wnd = attempt_set_foreground(target_window, orig_foreground_wnd);
	} // if()

	// Very important to detach any threads whose inputs were attached above,
	// prior to returning, otherwise the next attempt to attach thread inputs
	// for these particular windows may result in a hung thread or other
	// undesirable effect:
	if is_attached_my_to_fore.as_bool() {
		AttachThreadInput(current_thread_id, fore_thread, BOOL(0));
    }
	if is_attached_fore_to_target.as_bool() {
		AttachThreadInput(fore_thread, target_thread, BOOL(0));
    }
    
	// Finally.  This one works, solving the problem of the MessageBox window
	// having the input focus and being the foreground window, but not actually
	// being visible (even though IsVisible() and IsIconic() say it is)!  It may
	// help with other conditions under which this function would otherwise fail.
	// Here's the way the repeat the failure to test how the absence of this line
	// affects things, at least on my XP SP1 system:
	// y::MsgBox, test
	// #e::(some hotkey that activates Windows Explorer)
	// Now: Activate explorer with the hotkey, then invoke the MsgBox.  It will
	// usually be activated but invisible.  Also: Whenever this invisible problem
	// is about to occur, with or without this fix, it appears that the OS's z-order
	// is a bit messed up, because when you dismiss the MessageBox, an unexpected
	// window (probably the one two levels down) becomes active rather than the
	// window that's only 1 level down in the z-order:
	if !new_foreground_wnd.is_invalid() {
         // success.
		// Even though this is already done for the IE 5.5 "hack" above, must at
		// a minimum do it here: The above one may be optional, not sure (safest
		// to leave it unless someone can test with IE 5.5).
		// Note: I suspect the two lines below achieve the same thing.  They may
		// even be functionally identical.  UPDATE: This may no longer be needed
		// now that the first BringWindowToTop(), above, has been disabled due to
		// its causing more trouble than it's worth.  But seems safer to leave
		// this one enabled in case it does resolve IE 5.5 related issues and
		// possible other issues:
		BringWindowToTop(target_window);
		//SetWindowPos(target_window, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
		return new_foreground_wnd; // Return this rather than target_window because it's more appropriate.
	} else {
		return HWND(0);
    }
    //  And if that doesn't work
    // https://stackoverflow.com/questions/17782622/understanding-systemparametersinfo-spi-setforegroundlocktimeout
    /*
    
void SetForegroundLockTimeout()
{
	// Even though they may not help in all OSs and situations, this lends peace-of-mind.
	// (it doesn't appear to help on my XP?)
	if (g_os.IsWin98orLater() || g_os.IsWin2000orLater())
	{
		// Don't check for failure since this operation isn't critical, and don't want
		// users continually haunted by startup error if for some reason this doesn't
		// work on their system:
		if (SystemParametersInfo(SPI_GETFOREGROUNDLOCKTIMEOUT, 0, &g_OriginalTimeout, 0))
			if (g_OriginalTimeout) // Anti-focus stealing measure is in effect.
			{
				// Set it to zero instead, disabling the measure:
				SystemParametersInfo(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, (PVOID)0, SPIF_SENDCHANGE);
//				if (!SystemParametersInfo(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, (PVOID)0, SPIF_SENDCHANGE))
//					MsgBox("Enable focus-stealing: set-call to SystemParametersInfo() failed.");
			}
//			else
//				MsgBox("Enable focus-stealing: it was already enabled.");
//		else
//			MsgBox("Enable focus-stealing: get-call to SystemParametersInfo() failed.");
	}
//	else
//		MsgBox("Enable focus-stealing: neither needed nor supported under Win95 and WinNT.");
}


*/
}


pub fn set_foreground_window_ex(target_window: HWND) -> HWND {
    unsafe {
        _set_foreground_window_ex(target_window)
    }
}

fn set_foreground_window2(windowh: HWND) -> windows::core::Result<()> {
    unsafe {
        let foreground = GetForegroundWindow();

        let foreground_thread_id = GetWindowThreadProcessId(foreground, std::ptr::null_mut() as *mut u32);
        let target_thread_id = GetWindowThreadProcessId(windowh, std::ptr::null_mut() as *mut _);
        AttachThreadInput(target_thread_id, foreground_thread_id, BOOL(1)).ok()?;

        SetForegroundWindow(windowh);
        BringWindowToTop(windowh);
        SetFocus(windowh);

        AttachThreadInput(target_thread_id, foreground_thread_id, BOOL(0)).ok()?;

        Ok(())
    }
}

// This code is taken from IslandWindow::_globalActivateWindow from windows terminal.
pub fn set_foreground_window_terminal(windowh: HWND) -> windows::core::Result<()> {
    unsafe {
        if !IsWindowVisible(windowh).as_bool() {
            ShowWindow(windowh, SW_SHOW);
        }
        ShowWindow(windowh, SW_RESTORE);

        let foreground = GetForegroundWindow();

        let foreground_thread_id = GetWindowThreadProcessId(foreground, std::ptr::null_mut() as *mut u32);
        let current_thread_id = GetCurrentThreadId();

        AttachThreadInput(foreground_thread_id, current_thread_id, BOOL(1)).ok()?;

        BringWindowToTop(windowh);
        ShowWindow(windowh, SW_SHOW);
        SetActiveWindow(windowh);

        AttachThreadInput(foreground_thread_id, current_thread_id, BOOL(0)).ok()?;

        Ok(())
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