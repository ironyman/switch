use windows::Win32::Foundation::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::SystemServices::*;
use windows::Win32::System::Memory::*;

pub fn get_text() -> String {
    unsafe {
        OpenClipboard(HWND(0));
        let data_handle = GetClipboardData(CF_TEXT.0);
        if data_handle.is_invalid() {
            return "".into();
        }

        let data = GlobalLock(data_handle.0);
        // let text = String::from_utf8_lossy(std::mem::transmute(data)).into() as String;
        let cstr = std::ffi::CStr::from_ptr(data as *const _);
        let text = cstr.to_str().unwrap();

        GlobalUnlock(data_handle.0);

        CloseClipboard();
        return text.into();
    }
}