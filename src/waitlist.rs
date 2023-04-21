use windows::{
    Win32::Foundation::*,
    Win32::System::WindowsProgramming::*,
    Win32::UI::WindowsAndMessaging::*,
};

pub struct WaitList {
    handles: std::sync::Arc<std::sync::Mutex<Vec<HANDLE>>>,
}

pub enum WaitResult {
    Handle(HANDLE),
    Message,
    Error(u32)
}

unsafe impl Send for WaitList {}
unsafe impl Sync for WaitList {}

impl WaitList {
    pub fn new() -> WaitList {
        WaitList{
            handles: std::sync::Arc::new(std::sync::Mutex::new(vec![]))
        }
    }

    pub fn add(&mut self, handle: HANDLE) -> bool {
        if handle.is_invalid() {
            return false;
        }
        let mut handles = self.handles.lock().unwrap();
        handles.push(handle);
        return true;
    }

    pub fn remove(&mut self, handle: HANDLE) -> bool {
        let mut handles = self.handles.lock().unwrap();

        let index = handles.iter().position(|&h| h == handle);
        if index == None {
            return false;
        }

        handles.swap_remove(index.unwrap());

        return true;
    }

    pub fn wait(&self) -> WaitResult {
        unsafe {
            let handles = self.handles.lock().unwrap();
            let signalled = MsgWaitForMultipleObjects(handles.as_ref(), BOOL(0), INFINITE, QS_ALLINPUT) as usize;
            if signalled < handles.len() {
                return WaitResult::Handle(handles[signalled]);
            } else if signalled == handles.len() {
                return WaitResult::Message;
            } else {
                return WaitResult::Error(signalled as u32);
            }
        }
    }

    pub fn waiter(&self) -> Box<dyn FnOnce() -> WaitResult> {
        let handles = self.handles.lock().unwrap().clone();
        Box::new(move || {
            unsafe {
                let signalled = MsgWaitForMultipleObjects(handles.as_ref(), BOOL(0), INFINITE, QS_ALLINPUT) as usize;
                if signalled < handles.len() {
                    return WaitResult::Handle(handles[signalled]);
                } else if signalled == handles.len() {
                    return WaitResult::Message;
                } else {
                    return WaitResult::Error(signalled as u32);
                }
            }
        })
    }
}