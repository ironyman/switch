use windows::Win32::System::Ole::*;
// use windows::Win32::Globalization::*;
use std::mem::ManuallyDrop;
use windows::Win32::System::Com::*;
use windows::Win32::Foundation::*;

pub struct Variant(pub VARIANT);
impl Variant {
    pub fn new(num: VARENUM, contents: VARIANT_0_0_0) -> Variant {
        Variant {
            0: VARIANT {
                Anonymous: VARIANT_0 {
                    Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                        vt: num.0 as u16,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: contents,
                    }),
                },
            },
        }
    }   
}

impl From<String> for Variant {
    fn from(value: String) -> Variant { Variant::new(VT_BSTR, VARIANT_0_0_0 { bstrVal: ManuallyDrop::new(BSTR::from(value)) }) }
}

impl From<u32> for Variant {
    fn from(value: u32) -> Variant { Variant::new(VT_UINT, VARIANT_0_0_0 { ulVal: value }) }
}

impl From<i32> for Variant {
    fn from(value: i32) -> Variant { Variant::new(VT_INT, VARIANT_0_0_0 { lVal: value }) }
}

impl Drop for Variant {
    fn drop(&mut self) {
        match VARENUM(unsafe { self.0.Anonymous.Anonymous.vt as i32 }) {
            VT_BSTR => unsafe {
                drop(&mut &self.0.Anonymous.Anonymous.Anonymous.bstrVal)
            } 
            _ => {}
        }
        unsafe { drop(&mut self.0.Anonymous.Anonymous) }
    }
}
