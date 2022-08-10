use std::io::Write;

use windows::Win32::System::Environment::*;
use windows::Win32::System::WindowsProgramming::*;
use windows::Win32::NetworkManagement::NetManagement::*;
use windows::Win32::Security::*;
use windows::Win32::Security::Authorization::*;
use windows::Win32::System::Memory::*;
use windows::core::*;
use windows::Win32::System::WinRT::*;
use windows::Win32::System::Com::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Shell::*;
// Used implicitly.
// use windows::Management::Deployment::*;
use switch::log::*;
use switch::startappsprovider::{AppEntry, AppEntryKind};

// const DEFAULT_THREADS: u32 = 256;

struct IndexRoot {
    path: &'static str,
    // path2: &'static std::ffi::OsStr,
    // kind: AppKind,
    max_depth: i32,
}
// Appx should be queried with winrt and not by fs enumeration.
// enum AppKind {
//     Exe,
//     Appx,
// }

const INDEX_DIRECTORIES: &'static [IndexRoot] = &[
    IndexRoot {
        path: "%ProgramData%\\Microsoft\\Windows\\Start Menu\\\0",
        // kind: AppKind::Exe,
        max_depth: 99,
    },
    IndexRoot {
        path: "%USERPROFILE%\\.cargo\\bin\\\0",
        // kind: AppKind::Exe,
        max_depth: 99,
    },
    IndexRoot {
        path: "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\\0",
        // kind: AppKind::Exe,
        max_depth: 99,
    },
    IndexRoot {
        path: "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\\0",
        // kind: AppKind::Exe,
        max_depth: 0,
    },
    IndexRoot {
        path: "%ProgramData%\\chocolatey\\bin\\\0",
        // kind: AppKind::Exe,
        max_depth: 99,
    },
    IndexRoot {
        path: "%SystemRoot%\\\0",
        // path2: std::ffi::OsStr::new("%SystemRoot%\\system32\\"),
        // kind: AppKind::Exe,
        max_depth: 0,
    },
    IndexRoot {
        path: "%SystemRoot%\\system32\\\0",
        // path2: std::ffi::OsStr::new("%SystemRoot%\\system32\\"),
        // kind: AppKind::Exe,
        max_depth: 0,
    },
    // IndexRoot {
    //     path: "%ProgramFiles%\\WindowsApps\\\0",
    //     kind: AppKind::Appx,
    //     max_depth: 0,
    // },
    // These are not apps users would want to run...
    // IndexRoot {
    //     path: "%SystemRoot%\\SystemApps\\\0",
    //     kind: AppKind::Appx,
    //     max_depth: 0,
    // },
];

fn visit_directories<IntoPath>(root: IntoPath, cb: &mut dyn FnMut(&std::fs::DirEntry), max_depth: i32) -> std::io::Result<()>
where IntoPath: Into<std::path::PathBuf> {
    if max_depth < 0 {
        return Ok(());
    }

    let path = root.into();
    // switch::trace!("indexer", log::Level::Info, "Visiting {:?} available depth {}", path, max_depth);
    let dirs = std::fs::read_dir(&path)?;

    for entry in dirs {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_directories(&path, cb, max_depth - 1)?;
        } else {
            cb(&entry);
        }
    }

    return Ok(());
}

fn save_apps(apps: &Vec<AppEntry>) -> anyhow::Result<()> {
    let path = switch::path::get_app_data_path("apps.json")?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(serde_json::to_string(&apps)?.as_bytes())?;
    file.sync_all()?;
    return Ok(());
}

fn index_exes() -> anyhow::Result<Vec<AppEntry>> {
    let mut apps: Vec<AppEntry> = vec![];

    let mut gather_exes = |de: &std::fs::DirEntry| {
        // let valid_extensions = ["exe", "lnk", "msc", "cpl"];
        let extension: String = de.path().extension().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().into();
        // if !valid_extensions.contains(&&extension[..]) {
        //     return;
        // } 

        match &extension[..] {
            "exe" | "msc" | "cpl" | "appref-ms" => {
                apps.push(AppEntry {
                    name: de.path().file_name().unwrap_or(std::ffi::OsStr::new("None")).to_str().unwrap().into(),
                    kind: AppEntryKind::Exe {
                        path: de.path().to_str().unwrap().into(),
                        params: String::new(),
                    },
                    ..Default::default()
                });
            },
            "lnk" => {
                unsafe {
                    let shell = CoCreateInstance::<_, IShellDispatch>(
                        &windows::core::GUID::from_u128(0x13709620_C279_11CE_A49E_444553540000), // CLSID_Shell
                        None,
                        CLSCTX_INPROC_SERVER).unwrap();
                    let dir = de.path().parent().unwrap().to_str().unwrap().to_owned() + "\0";
                    // let dir = dir.encode_utf16().collect::<Vec<u16>>();
                    let dir = switch::com::Variant::from(dir);

                    let folder = match shell.NameSpace(&dir.0) {
                        Ok(f) => {
                            f
                        },
                        _ => {
                            return
                        }
                    };

                    let file = de.file_name().to_str().unwrap().to_owned() + "\0";
                    let file = file.encode_utf16().collect::<Vec<u16>>();
                    let item = match folder.ParseName(BSTR::from_wide(&file)) {
                        Ok(item) => {
                            item
                        },
                        _ => {
                            return
                        }
                    };

                    let link = match item.GetLink() {
                        Ok(link) => {
                        // /// Attempts to cast the current interface to another interface using `QueryInterface`.
                        // /// The name `cast` is preferred to `query` because there is a WinRT method named query but not one
                        // /// named cast.
                        // fn cast<T: Interface>(&self) -> Result<T> {
                        //     unsafe {
                        //         let mut result = None;

                        //         (self.assume_vtable::<IUnknown>().QueryInterface)(core::mem::transmute_copy(self), &T::IID, &mut result as *mut _ as _).and_some(result)
                        //     }
                        // }
                            link.cast::<IShellLinkDual>().unwrap()
                        },
                        _ => {
                            return
                        }
                    };


                    // convert PCSTR to str, lstrlenA uses use windows::Win32::Globalization::*, and "Win32_Globalization"
                    // let mut target_path = [0u8; 512];
                    // let _ = link.GetPath(&mut target_path, std::ptr::null_mut(), 0);
                    // let len = lstrlenA(PCSTR(target_path.as_ptr()));
                    // let target_path = std::str::from_utf8(&target_path[0..len as usize]).unwrap();
                    
                    apps.push(AppEntry {
                        name: de.path().file_name().unwrap_or(std::ffi::OsStr::new("None")).to_str().unwrap().into(),
                        kind: AppEntryKind::Link {
                            path: de.path().to_str().unwrap().into(),
                            params: String::new(),
                            target_path: link.Path().unwrap().to_string()
                        },
                        ..Default::default()
                    });
                }
            },
            _ => {
            }
        }
    };

    for root in INDEX_DIRECTORIES {
        let expanded_path: String;
        
        unsafe {
            let mut expanded: [u8; 512] = [0; 512];
            // PSTR(expanded.as_mut_ptr())
            let len = ExpandEnvironmentStringsA(PCSTR(root.path.as_ptr()), &mut expanded) as usize;
            // Exclude null terminator which is needed for ExpandEnvironmentStringsA but not for rust strings.
            expanded_path = String::from_utf8_lossy(&expanded[..len-1]).into();
        }
        switch::trace!("indexer", log::Level::Info, "Indexing {:?}", expanded_path);
        if let Err(err) = visit_directories(expanded_path, &mut gather_exes, root.max_depth) {
            switch::trace!("indexer", log::Level::Error, "Error: {:?}", err);
        }
    }

    return Ok(apps);
}

// Activating this factory requires high integrity level some reason.
unsafe fn index_appx() -> anyhow::Result<Vec<AppEntry>> {
    let mut apps: Vec<AppEntry> = vec![];
    RoInitialize(RO_INIT_SINGLETHREADED)?;
    
    // https://github.com/tpn/winsdk-10/blob/master/Include/10.0.16299.0/winrt/windows.management.deployment.h
    // definition of RuntimeClass_Windows_Management_Deployment_PackageManager is following
    let class_id = "Windows.Management.Deployment.PackageManager".encode_utf16().collect::<Vec<u16>>();
    let class_id = WindowsCreateString(&class_id)?;
    let af: windows::core::IActivationFactory = RoGetActivationFactory(class_id)?;
    // let pi: windows::core::IInspectable = af.ActivateInstance()?;
    // let pm: windows::Management::Deployment::IPackageManager = pi.into();
    let pm: windows::Management::Deployment::PackageManager = af.ActivateInstance()?;

    // This requires high integrity.
    // let packages = pm.FindPackages()?;

    let mut username: [u8; UNLEN as usize + 1] = [0; UNLEN as usize + 1];
    let mut username_length = UNLEN + 1 as u32;
    GetUserNameA(PSTR(username.as_mut_ptr()), &mut username_length);

    let mut sid_length = 0u32;
    let mut domain_length = 0u32;
    LookupAccountNameA(
        PCSTR(std::ptr::null()),
        PCSTR(username.as_ptr()),
        PSID(0),
        &mut sid_length,
        PSTR(std::ptr::null_mut()),
        &mut domain_length,
        std::ptr::null_mut(),
    );

    let mut sid_buf: Vec<u8> = vec![0; sid_length as usize];
    let mut domain_buf: Vec<u8> = vec![0; domain_length as usize];
    let mut peuse = SID_NAME_USE(0);
    LookupAccountNameA(
        PCSTR(std::ptr::null()),
        PCSTR(username.as_ptr()),
        PSID(sid_buf.as_mut_ptr() as isize),
        &mut sid_length,
        PSTR(domain_buf.as_mut_ptr()),
        &mut domain_length,
        &mut peuse,
    );

    let mut string_sid_ptr = PSTR(std::ptr::null_mut());
    ConvertSidToStringSidA(PSID(sid_buf.as_mut_ptr() as isize), &mut string_sid_ptr);

    let string_sid_cstr = std::ffi::CStr::from_ptr(string_sid_ptr.0 as *const _);
    let string_sid = string_sid_cstr.to_str().unwrap();
    let packages = pm.FindPackagesByUserSecurityId(windows::core::HSTRING::from(string_sid))?;
    LocalFree(std::mem::transmute(string_sid_ptr.0));
    std::mem::forget(string_sid_ptr);
    std::mem::forget(string_sid_cstr);

    for p in packages {
        // Oh my rust...

        // println!("{:?}", p.DisplayName()?);
        // println!("{:?}", p.Id()?.Name());
        // println!("{:?}", p.Id()?.PublisherId());

        let path = match p.InstalledPath() {
            Ok(path) => path,
            Err(_) => continue,
        };

        let display_name = match p.DisplayName() {
            Ok(name) => name,
            Err(_) => continue,
        };

        let identity_id  = match p.Id() {
            Ok(p) => match p.Name() {
                Ok(name) => name,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let publisher_id  = match p.Id() {
            Ok(p) => match p.PublisherId() {
                Ok(id) => id,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        for app in p.GetAppListEntries()? {
            // println!("{:?}", app.AppInfo()?.Id()?.to_string());
            let app_id = match app.AppInfo() {
                Ok(appinfo) => match appinfo.Id() {
                    Ok(id) => id.to_string(),
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            apps.push(AppEntry {
                name: display_name.to_string_lossy() + " (" + &app_id + ")",
                kind: AppEntryKind::Appx {
                    identity_id: identity_id.to_string_lossy(),
                    publisher_id: publisher_id.to_string_lossy(),
                    application_id: app_id,
                    path: path.to_string_lossy(),
                },
                ..Default::default()
            });
        }
    }
    return Ok(apps);
}

fn main() -> anyhow::Result<()> {
    switch::log::initialize_test_log(log::Level::Debug, &["indexer"]).unwrap();

    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED).ok();
    }

    let mut apps = index_exes()?;
    apps.append(unsafe { &mut index_appx()? });
    save_apps(&apps)?;
    return Ok(());
}