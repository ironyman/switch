
// cargo test --package switch --test pipesecurity -- openpipe --nocapture
#[test]
fn openpipe() {
    unsafe {
    let file = windows::Win32::Storage::FileSystem::CreateFileA(
        windows::core::PCSTR("\\\\.\\Pipe\\QuakeTerminalRunner\0".as_ptr()),
        windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
        windows::Win32::Storage::FileSystem::FILE_SHARE_NONE,
        std::ptr::null(),
        windows::Win32::Storage::FileSystem::OPEN_EXISTING,
        windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        windows::Win32::Foundation::HANDLE(0));

    println!("Handle: {}, error: {:?} {}", file.0, windows::Win32::Foundation::GetLastError(), windows::core::Error::from_win32());
    if !file.is_invalid() {
        windows::Win32::Foundation::CloseHandle(file);
    }
    }
}
