use windows::{
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::HiDpi::*,
    // Win32::UI::Shell::*,
    Win32::Graphics::Dwm::*,
    Win32::Graphics::Gdi::*,
    // Win32::System::Com::*,
    // core::Interface,
    // core::*,
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

// TODO: great now we have two of these WindowInfo types
// and two ways of enumerating windows. Please fix.
// #[derive (Debug)] 
pub struct WindowInfo {
    hwnd: HWND,
    rc: RECT,
    z: i32,
    visible_region: HRGN,
    visible_percent: i32,
    visible_centroid: POINT,
    window_text: String,
}

impl Clone for WindowInfo {
    fn clone(&self) -> Self {
        let bytes = unsafe { GetRegionData(self.visible_region, 0, std::ptr::null_mut()) };

        // https://stackoverflow.com/questions/66611678/malloc-an-array-with-rust-layout
        let buf = vec![0u8; bytes as usize];
        let mut region: Box<RGNDATA> = unsafe { std::mem::transmute(buf.as_ptr()) };
        std::mem::forget(buf);

        unsafe { GetRegionData(self.visible_region, bytes, region.as_mut()); }
        let cloned_region = unsafe { ExtCreateRegion(std::ptr::null(), bytes, region.as_ref()) };

        let new = WindowInfo {
            hwnd: self.hwnd,
            rc: self.rc,
            z: self.z,
            visible_region: cloned_region,
            visible_percent: self.visible_percent,
            visible_centroid: self.visible_centroid,
            window_text: self.window_text.clone(),
        };

        return new;
    }

}

impl Drop for WindowInfo {
    fn drop(&mut self) {
        if !self.visible_region.is_invalid() {
            unsafe { DeleteObject(self.visible_region) };
        }
    }
}

impl WindowInfo {
    // From high z-order to lowest.
    fn iter_front_to_back() -> WindowIterator {
        return WindowIterator {
            current: unsafe { GetTopWindow(GetDesktopWindow()) },
            direction: GW_HWNDNEXT,
        }
    }

    // From low z-order to highest.
    fn iter_back_to_front() -> WindowIterator {
        return WindowIterator {
            current: unsafe { GetWindow(GetTopWindow(GetDesktopWindow()), GW_HWNDLAST) },
            direction: GW_HWNDPREV,
        }
    }

    // Return None if the window is non interactive i.e. not switch-to-able
    fn create_from_hwnd(hwnd: HWND) -> Option<WindowInfo> {
        if !unsafe { is_window_visible(hwnd) } {
            return None;
        }
        let mut rc: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(hwnd, &mut rc); }
        if rc.right - rc.left == 0 || rc.bottom - rc.top == 0 {
            return None;
        }

        let mut window_text: [u16; 512] = [0; 512];
        let len = unsafe { GetWindowTextW(hwnd, &mut window_text) };
        if len == 0 {
            return None;
        }
        let window_text = String::from_utf16_lossy(&window_text[..len as usize]);
        return Some(WindowInfo {
            hwnd,
            rc,
            z: 0,
            visible_region: unsafe { CreateRectRgn(rc.left, rc.top, rc.right, rc.bottom) },
            visible_percent: 0,
            visible_centroid: POINT::default(),
            window_text,
        });
    }
}

impl std::fmt::Display for WindowInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.window_text)
    }
}

impl std::fmt::Debug for WindowInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} ({})", self.window_text, self.visible_percent)
    }
}

struct WindowIterator {
    current: HWND,
    direction: GET_WINDOW_CMD,
}

impl WindowIterator {    
    fn next_window(&self) -> HWND {
        return unsafe { GetWindow(self.current, self.direction) };
    }
}

impl Iterator for WindowIterator {
    type Item = HWND;

    fn next(&mut self) -> Option<Self::Item> {
        self.current = self.next_window();
        if self.current.is_invalid() {
            return None;
        }
        return Some(self.current);
    }
}

unsafe fn get_candidate_windows_old() -> Vec<WindowInfo> {
    let mut result: Vec<WindowInfo> = vec![];
    let mut hwnd = GetTopWindow(GetDesktopWindow());
    let mut z = 0;
    while !hwnd.is_invalid() {
        if is_window_visible(hwnd) {
            let mut rc: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rc);
            if rc.right - rc.left != 0 && rc.bottom - rc.top != 0 {

                let mut window_text: [u16; 512] = [0; 512];
                let len = GetWindowTextW(hwnd, &mut window_text);
                let window_text = String::from_utf16_lossy(&window_text[..len as usize]);

                result.push(WindowInfo {
                    hwnd,
                    rc,
                    z,
                    visible_region: CreateRectRgn(rc.left, rc.top, rc.right, rc.bottom),
                    visible_percent: 0,
                    visible_centroid: POINT::default(),
                    window_text,
                });
                z = z - 1;
            }
        }
        hwnd = GetWindow(hwnd, GW_HWNDNEXT);
    }

    return result;
}

pub unsafe fn get_candidate_windows() -> Vec<WindowInfo> {
    // let mut result: Vec<WindowInfo> = vec![];

    // for window_handle in WindowInfo::iter_front_to_back() {
    //     // println!("{}", window_handle.0);
    //     if let Some(w) = WindowInfo::create_from_hwnd(window_handle) {
    //         result.push(w);
    //     }
    // }   
    // return result;

    return WindowInfo::iter_front_to_back().filter_map(WindowInfo::create_from_hwnd).collect::<Vec<WindowInfo>>();
}


pub unsafe fn highlight_window(window: HWND) {
    let cx_border = GetSystemMetrics(SM_CXBORDER);
    let hdc = GetDC(HWND(0));

    let pen = CreatePen(PS_INSIDEFRAME, 3*cx_border, 0);
    let old_pen = SelectObject(hdc, pen);
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    
    SetROP2(hdc, R2_NOT);

    let mut rc: RECT = std::mem::zeroed();
    GetWindowRect(window, &mut rc);
    if rc.right - rc.left == 0 || rc.bottom - rc.top == 0 {
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        DeleteObject(pen);
        ReleaseDC(HWND(0), hdc);
        return;
    }

    // https://docs.microsoft.com/en-us/windows/win32/learnwin32/dpi-and-device-independent-pixels
    // Window rect units in DIPS, GDI units  in pixels, convert to pixels.
    let dpi_scale = GetDpiForWindow(window) as f32 / 96f32;
    rc.left = (rc.left as f32 * dpi_scale) as i32;
    rc.right = (rc.right as f32 * dpi_scale) as i32;
    rc.top = (rc.top as f32 * dpi_scale) as i32;
    rc.bottom = (rc.bottom as f32 * dpi_scale) as i32;

    crate::trace!("directional_switching", log::Level::Debug, "highlight_window: {:?}", rc);
    
    Rectangle(hdc, rc.left, rc.top, rc.right, rc.bottom);

    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(pen);
    ReleaseDC(HWND(0), hdc);
}

pub unsafe fn highlight_window3(window: HWND, invalidate: bool) {
    let hdc = GetDC(HWND(0));

    let mut rc: RECT = std::mem::zeroed();
    GetWindowRect(window, &mut rc);
    if rc.right - rc.left == 0 || rc.bottom - rc.top == 0 {
        return;
    }

    // https://docs.microsoft.com/en-us/windows/win32/learnwin32/dpi-and-device-independent-pixels
    // Window rect units in DIPS, GDI units  in pixels, convert to pixels.
    let dpi_scale = GetDpiForWindow(window) as f32 / 96f32;
    rc.left = (rc.left as f32 * dpi_scale) as i32;
    rc.right = (rc.right as f32 * dpi_scale) as i32;
    rc.top = (rc.top as f32 * dpi_scale) as i32;
    rc.bottom = (rc.bottom as f32 * dpi_scale) as i32;

    crate::trace!("directional_switching", log::Level::Debug, "highlight_window: {:?}", rc);
    
    // https://docs.microsoft.com/en-us/windows/win32/gdi/colorref
    let black = CreateSolidBrush(0);

    if !invalidate {
        FrameRect(hdc, &rc, black);
    } else {
        // InvalidateRect causes window to flash as it's repainted, undesirable
        // PostMessageA(adjacent_window, WM_PAINT, WPARAM(0), LPARAM(0));
        InvalidateRect(HWND(0), &rc, BOOL(0));
    }

    // FillRect(hdc, &rc, black);
    ReleaseDC(HWND(0), hdc);
}

pub unsafe fn highlight_window2(window: HWND) {
    // https://www.codeproject.com/Articles/1988/Guide-to-WIN32-Paint-for-beginners
    // https://docs.microsoft.com/en-us/windows/win32/gdi/using-multiple-monitors-as-independent-displays?redirectedfrom=MSDN
    // https://stackoverflow.com/questions/695897/drawing-over-all-windows-on-multiple-monitors
    let hdc = GetDC(HWND(0));
    // let hdc = CreateDCA(PCSTR(b"\\\\.\\DISPLAY1".as_ptr()), PCSTR(std::ptr::null()), PCSTR(std::ptr::null()), std::ptr::null());
    // let hdc = CreateDCA(PCSTR(b"\\\\.\\DISPLAY".as_ptr()), PCSTR(std::ptr::null()), PCSTR(std::ptr::null()), std::ptr::null());
    // None of those dcs make a difference.

    let mut rc: RECT = std::mem::zeroed();
    GetWindowRect(window, &mut rc);
    if rc.right - rc.left == 0 || rc.bottom - rc.top == 0 {
        return;
    }

    crate::trace!("directional_switching", log::Level::Debug, "highlight_window: {:?}", rc);
    rc.left = 0;
    rc.right = 1440;
    rc.top = 0;
    rc.bottom = 960;
    // https://docs.microsoft.com/en-us/windows/win32/gdi/colorref
    let black = CreateSolidBrush(0);

    FillRect(hdc, &rc, black);
    // FrameRect(hdc, &rc, black);
    ReleaseDC(HWND(0), hdc);
}

pub unsafe fn get_adjacent_window(from_window: HWND, dir: Direction) -> anyhow::Result<HWND> {
    let mut windows = get_candidate_windows();

    crate::trace!("directional_switching", log::Level::Debug, "get_candidate_windows: {:?}", windows);

    calculate_visibility(&mut windows);
    
    let from_window_info = windows.iter().find(|&w| w.hwnd == from_window)
        .ok_or(anyhow::Error::msg("From window not found"))?;

    crate::trace!("directional_switching", log::Level::Debug, "from_window_info: {:?}", from_window_info);

    let mut adjacent_angle = 0f32;

    let adjacent = windows.iter()
        .filter(|&w| w.visible_percent >= 50)
        .filter(|&w| w.hwnd != from_window)
        .filter(|&w| !IsIconic(w.hwnd).as_bool())
        .fold(None, |accumulator: Option<&WindowInfo>, item| {
        // if (dir == Direction::Left && item.visible_centroid.x > from_window_info.visible_centroid.x) ||
        //     (dir == Direction::Right && item.visible_centroid.x < from_window_info.visible_centroid.x) ||
        //     (dir == Direction::Up && item.visible_centroid.y > from_window_info.visible_centroid.y) ||
        //     (dir == Direction::Down && item.visible_centroid.y > from_window_info.visible_centroid.y) {
        //     return accumulator;
        // }

        // if item.visible_percent < 50 {
        //     return accumulator;
        // }

        // if item.hwnd == from_window_info.hwnd {
        //     return accumulator;
        // } else if accumulator.hwnd == from_window_info.hwnd {
        //     return item;
        // }

        let direction_vector = match dir {
            Direction::Left => POINT { x: -1, y: 0 },
            Direction::Right => POINT { x: 1, y: 0 },
            Direction::Up => POINT { x: 0, y: -1 },
            Direction::Down => POINT { x: 0, y: 1 },
        };

        let item_vector = POINT {
            x: item.visible_centroid.x - from_window_info.visible_centroid.x,
            y: item.visible_centroid.y - from_window_info.visible_centroid.y,
        };

        let angle = cosine_angle(&item_vector, &direction_vector);
        crate::trace!("directional_switching", log::Level::Debug, "considering: {:?}, {}", item, angle);

        if angle > std::f32::consts::FRAC_PI_2 - 0.1 {
            return accumulator;
        }

        if let None = accumulator {
            return Some(item);
        }

        let accumulator_distance = sqdist(&from_window_info.visible_centroid, &accumulator.unwrap().visible_centroid);
        let item_distance = sqdist(&from_window_info.visible_centroid, &item.visible_centroid);

        if accumulator_distance < item_distance {
            return accumulator;
        } else {
            adjacent_angle = cosine_angle(&item_vector, &direction_vector);
            crate::trace!("directional_switching", log::Level::Debug, "wtf: {}", cosine_angle(&item_vector, &direction_vector));
            return Some(item);
        }
    }).ok_or(anyhow::Error::msg("Adjacent window not found"))?;
    crate::trace!("directional_switching", log::Level::Debug, "adjacent: {:?}, {}", adjacent, adjacent_angle);

    return Ok(adjacent.hwnd);
    // let mut from_rc: RECT = std::mem::zeroed();
    // GetWindowRect(from_window, &mut from_rc);

    // crate::trace!("directional_switching", log::Level::Debug, "from_rc: {:?}", from_rc);

    // for w in windows.iter() {
    //     if is_window_visible(candidate_window) {
    //         let mut candidate_rc: RECT = std::mem::zeroed();
    //         GetWindowRect(candidate_window, &mut candidate_rc);
    //         crate::trace!("directional_switching", log::Level::Debug, "candidate_rc: {:?}", candidate_rc);

    //         if dir == Direction::Left && candidate_rc.right <= from_rc.left {
    //             return Ok(candidate_window);
    //         } else if dir == Direction::Right && candidate_rc.left >= from_rc.right {
    //             return Ok(candidate_window);
    //         } else if dir == Direction::Up && candidate_rc.bottom <= from_rc.top {
    //             return Ok(candidate_window);
    //         } else if dir == Direction::Down && candidate_rc.top >= from_rc.bottom {
    //             return Ok(candidate_window);
    //         }
    //     }
    //     candidate_window = GetWindow(candidate_window, GW_HWNDNEXT);
    //     if candidate_window.is_invalid() {
    //         return Err(windows::core::Error::new(E_HANDLE, windows::core::HSTRING::from("No more windows")));
    //     }
    // }
    // Ok(HWND(0))
}

// cargo.exe test --package switch --lib -- windowgeometry::enumerate_windows --exact --nocapture
#[test]
fn enumerate_windows() {
    let mut windows = unsafe { get_candidate_windows() };
    crate::log::initialize_test_log(log::Level::Debug, &["directional_switching", "calculate_visibility"]).unwrap();
    crate::trace!("directional_switching", log::Level::Info, "found windows {:?}", windows);
    unsafe { calculate_visibility(&mut windows) };
    
    let visible_windows = windows.iter().filter(|&w| w.visible_percent > 49).cloned().collect::<Vec<WindowInfo>>();
    // println!("{:?}", visible_windows);
    crate::trace!("directional_switching", log::Level::Info, "visible windows {:?}", visible_windows);
}

fn rect_area(rc: &RECT) -> u64 {
    let width = (rc.right - rc.left) as u64;
    let height = (rc.bottom - rc.top) as u64;
    return width * height;
}

fn rect_centroid(rc: &RECT) -> POINT {
    return POINT { 
        x: (rc.right + rc.left)/2,
        y: (rc.bottom + rc.top)/2,
    };
}

// First average would be, avg = accumulate_average(0, 0, x0),
// then avg = accumulate_average(avg, 1, x1), etc, ...
fn accumulate_average(avg: u64, nr: usize, x: u64) -> u64 {
    // return (x + nr as u64 * avg) / (nr as u64 + 1);
    // that potentially overflows, but we don't need very high precision.
    let numerator = x as f32 + nr as f32 * avg as f32;
    let denominator = nr as f32 + 1.0;
    return (numerator / denominator) as u64;
}

// Since centroid of rect is averages along each dimension calculated independently
// we can calculate the centroid of bunch of rects using the cumulative average formula
// which is probably less efficient than the obvious way of doing it but easier to shoehorn
// into existing calculate_visibility function.
fn accumulate_centroid(average_centroid: &POINT, nr: usize, centroid: &POINT) -> POINT {
    return POINT {
        x: accumulate_average(average_centroid.x as u64, nr, centroid.x as u64) as i32,
        y: accumulate_average(average_centroid.y as u64, nr, centroid.y as u64) as i32,
    };
}

fn sqdist(a: &POINT, b: &POINT) -> u64 {
    let x = a.x as i64 - b.x as i64;
    let y = a.y as i64 - b.y as i64;
    return (x*x + y*y) as u64;
}

fn cosine_angle(a: &POINT, b: &POINT) -> f32 {
    let ax = a.x as f32;
    let ay = a.y as f32;
    let bx = b.x as f32;
    let by = b.y as f32;
    let numerator = ax*bx + ay*by;
    let denominator = (ax*ax + ay*ay).sqrt()*(bx*bx + by*by).sqrt();
    return f32::acos(numerator / denominator);
}

unsafe fn calculate_visibility(windows: &mut Vec<WindowInfo>) {   
    // Get visible_region, iterate from back to front
    for i in (0 .. windows.len()).rev() {
        for j in i + 1 .. windows.len() {
            let mut difference: RECT = std::mem::zeroed();

            if IntersectRect(&mut difference, &windows[j].rc, &windows[i].rc).as_bool() {
                let difference_rgn = CreateRectRgn(difference.left, difference.top, difference.right, difference.bottom);
                CombineRgn(windows[j].visible_region, windows[j].visible_region, difference_rgn, RGN_DIFF);
                DeleteObject(difference_rgn);
            }
        }
    }

    // Get visible_percent
    for i in 0 .. windows.len() {
        let bytes = GetRegionData(windows[i].visible_region, 0, std::ptr::null_mut());

        // https://stackoverflow.com/questions/66611678/malloc-an-array-with-rust-layout
        let buf = vec![0u8; bytes as usize];
        let mut region: Box<RGNDATA> = std::mem::transmute(buf.as_ptr());
        std::mem::forget(buf);

        GetRegionData(windows[i].visible_region, bytes, region.as_mut());
        let total_area = rect_area(&windows[i].rc);
        let mut visible_area = 0u64;

        let rects_nr = region.rdh.nCount as usize;
        let rcs: &[RECT] = std::slice::from_raw_parts(region.Buffer.as_ptr() as *const _, rects_nr);

        let mut centroid_x_sum = 0i64;
        let mut centroid_y_sum = 0i64;
        for j in 0usize .. rects_nr {
            let area = rect_area(&rcs[j]);
            visible_area += area;
            centroid_x_sum += area as i64 * rect_centroid(&rcs[j]).x as i64;
            centroid_y_sum += area as i64 * rect_centroid(&rcs[j]).y as i64;
            // accumulate_centroid(
            //     &windows[i].visible_centroid, 
            //     j,
            //     &);
        }

        // https://stackoverflow.com/questions/24478349/how-to-find-the-centroid-of-multiple-rectangles
        // If Ai is the area of rectangle i, and Ci is the centroid of rectangle i, then the centroid of all the rectangles taken together is just:
        // Sum(i = 1..n; Ai Ci)/Sum(i = 1..n; Ai)
        if visible_area != 0 {
            windows[i].visible_centroid.x = (centroid_x_sum / visible_area as i64) as i32;
            windows[i].visible_centroid.y = (centroid_y_sum / visible_area as i64) as i32;
        }

        windows[i].visible_percent = (100 * visible_area / total_area) as i32;
        crate::trace!("calculate_visibility", log::Level::Info, "{} size {} {}, {:?}", windows[i].window_text, visible_area, total_area, windows[i].visible_centroid);
        std::mem::forget(rcs);
    }
}
