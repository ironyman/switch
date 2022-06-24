use windows::{
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    // Win32::UI::Shell::*,
    Win32::Graphics::Dwm::*,
    Win32::Graphics::Gdi::*,
    // Win32::System::Com::*,
    // core::Interface,
};

use crate::log::*;

#[derive(Eq, PartialEq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

unsafe fn is_window_cloaked(window: HWND) -> bool {
    let mut cloaked: BOOL = BOOL(0);
    let ok = DwmGetWindowAttribute(
        window,
        DWMWA_CLOAKED,
        core::mem::transmute(&mut cloaked),
        std::mem::size_of::<BOOL>() as u32).is_ok();
    return ok && cloaked.as_bool();
}

// TODO: virtual desktops
// unsafe fn is_on_current_virtual_desktop(window: HWND) -> bool {
//     CoCreateInstance(&IVirtualDesktopManager::IID, None, std::ptr::null(), CLSCTX_ALL);
//     let res = IVirtualDesktopManager::IsWindowOnCurrentVirtualDesktop(window);
//     return res.is_err() || res.unwrap().as_bool();
// }

unsafe fn is_normal_window(window: HWND) -> bool {
    let ex_style = WINDOW_EX_STYLE(GetWindowLongW(window, GWL_EXSTYLE) as u32);
    let style = WINDOW_STYLE(GetWindowLongW(window, GWL_STYLE) as u32);
    let mut window_text: [u16; 512] = [0; 512];
    let len = GetWindowTextW(window, &mut window_text);
    let window_text = String::from_utf16_lossy(&window_text[..len as usize]);

    return (ex_style & WS_EX_NOACTIVATE).0 == 0 &&
       (ex_style & WS_EX_TOOLWINDOW).0 == 0 &&
       (style & WS_POPUP).0 == 0 &&
       (style & WS_CHILD).0 == 0 &&
       window_text.len() != 0;
}

unsafe fn is_window_visible(window: HWND) -> bool {
    return IsWindowVisible(window).as_bool() && !is_window_cloaked(window) && is_normal_window(window);
}

struct WindowInfo {
    hwnd: HWND,
    rc: RECT,
    z: i32,
}

unsafe fn get_windows() -> Vec<WindowInfo> {
    let mut result: Vec<WindowInfo> = vec![];
    let mut hwnd = GetTopWindow(GetDesktopWindow());

    while !hwnd.is_invalid() {
        if !is_window_visible(hwnd) {
            let mut rc: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rc);
            result.push(WindowInfo {
                hwnd,
                rc,
                z: 0,
            });
        }
        hwnd = GetWindow(hwnd, GW_HWNDNEXT);
    }

    return result;
}

pub unsafe fn get_adjacent_window(from_window: HWND, dir: Direction) -> windows::core::Result<HWND> {
    let mut from_rc: RECT = std::mem::zeroed();
    GetWindowRect(from_window, &mut from_rc);

    let mut candidate_window = GetTopWindow(GetDesktopWindow());

    crate::trace!("directional_switching", log::Level::Debug, "from_rc: {:?}", from_rc);

    loop {
        if is_window_visible(candidate_window) {
            let mut candidate_rc: RECT = std::mem::zeroed();
            GetWindowRect(candidate_window, &mut candidate_rc);
            crate::trace!("directional_switching", log::Level::Debug, "candidate_rc: {:?}", candidate_rc);

            if dir == Direction::Left && candidate_rc.right <= from_rc.left {
                return Ok(candidate_window);
            } else if dir == Direction::Right && candidate_rc.left >= from_rc.right {
                return Ok(candidate_window);
            } else if dir == Direction::Up && candidate_rc.bottom <= from_rc.top {
                return Ok(candidate_window);
            } else if dir == Direction::Down && candidate_rc.top >= from_rc.bottom {
                return Ok(candidate_window);
            }
        }
        candidate_window = GetWindow(candidate_window, GW_HWNDNEXT);
        if candidate_window.is_invalid() {
            return Err(windows::core::Error::new(E_HANDLE, windows::core::HSTRING::from("No more windows")));
        }
    }
}

// // GetRgnBox
// unsafe fn get_visible_region(hwnd: HWND) -> HRGN {   
//     //Store the region of window hwnd
//     let mut rc: RECT = std::mem::zeroed();

//     GetWindowRect(hwnd, &mut rc);

//     let rgn = CreateRectRgn(rc.left, rc.top, rc.right, rc.bottom);

//     let parent = GetAncestor(hwnd, GA_PARENT);

//     let mut iter = hwnd;
//     let iter = hwnd;

//     while !iter.is_invalid() && iter != GetDesktopWindow() {
//         let topWnd = GetTopWindow(parent);
//         do
//         {
//             if topWnd == iter {
//                 break;
//             }
//             RECT topWndRect={0,0,0,0};
//             GetWindowRect(topWnd,&topWndRect);
//             RECT tempRect={0,0,0,0};
//             //Other window overlapping with hwnd
//             if(::IsWindowVisible(topWnd) && !::IsIconic(topWnd) && IntersectRect(&tempRect,&topWndRect, &rc)!=0) 
//             {
//                 HRGN topWndRgn=::CreateRectRgn(topWndRect.left,topWndRect.top,topWndRect.right,topWndRect.bottom);
//                 ::CombineRgn(rgn,rgn,topWndRgn,RGN_DIFF);
//                 ::RealDeleteObject(topWndRgn);
//             }
//             topWnd = GetNextWindow(topWnd, TWO);

//         } while(topWnd != NULL);

//         iter = parent;
//         parent = GetAncestor(parent, GA_PARENT);
//     }

//     return rgn;
// }
