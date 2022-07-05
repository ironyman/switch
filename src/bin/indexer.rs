use std::io::Write;

use windows::Win32::System::Environment::*;
use windows::core::*;
use windows::Win32::System::WinRT::*;
use windows::Management::Deployment::*;
use serde::{Serialize, Deserialize};
use switch::log::*;

// const DEFAULT_THREADS: u32 = 256;

struct IndexRoot {
    path: &'static str,
    // path2: &'static std::ffi::OsStr,
    kind: AppKind,
    max_depth: i32,
}

enum AppKind {
    Exe,
    Appx,
}

const INDEX_DIRECTORIES: &'static [IndexRoot] = &[
    IndexRoot {
        path: "%SystemRoot%\\system32\\\0",
        // path2: std::ffi::OsStr::new("%SystemRoot%\\system32\\"),
        kind: AppKind::Exe,
        max_depth: 0,
    },
    IndexRoot {
        path: "%ProgramData%\\Microsoft\\Windows\\Start Menu\\\0",
        kind: AppKind::Exe,
        max_depth: 99,
    },
    IndexRoot {
        path: "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\\0",
        kind: AppKind::Exe,
        max_depth: 99,
    },
    IndexRoot {
        path: "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\\0",
        kind: AppKind::Exe,
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

#[derive(Serialize, Deserialize, Debug)]
enum AppExecutableInfo {
    // Includes links, msc, exes, things you can pass path to ShellExecute to start
    Exe {
        path: String
    },
    // UWP/UAP apps, list with get-appxpackage, have to pass shell:AppsFolder\stuff to ShellExecute
    Appx {
        path: String,
        identity_id: String,
        publisher_id: String,
        application_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct AppEntry {
    name: String,
    exe_info: AppExecutableInfo,
}

impl Default for AppEntry {
    fn default() -> Self {
        return AppEntry { name: "".into(), exe_info: AppExecutableInfo::Exe { path: "".into() } };
    }
}

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
    let path = switch::log::get_app_data_path("apps.json")?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(serde_json::to_string(&apps)?.as_bytes())?;
    return Ok(());
}

fn index_exes() -> anyhow::Result<Vec<AppEntry>> {
    let mut apps: Vec<AppEntry> = vec![];

    let mut gather_exes = |de: &std::fs::DirEntry| {
        let valid_extensions = ["exe", "lnk", "msc"];
        let extension: String = de.path().extension().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().into();
        if !valid_extensions.contains(&&extension[..]) {
            return;
        } 
        apps.push(AppEntry {
            name: de.path().file_stem().unwrap_or(std::ffi::OsStr::new("None")).to_str().unwrap().into(),
            exe_info: AppExecutableInfo::Exe {
                path: de.path().to_str().unwrap().into(),
            }
        })
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
        visit_directories(expanded_path, &mut gather_exes, root.max_depth)?;
    }

    return Ok(apps);
}

unsafe fn index_appx() -> anyhow::Result<Vec<AppEntry>> {
    let mut apps: Vec<AppEntry> = vec![];
    RoInitialize(RO_INIT_SINGLETHREADED)?;
    
    // https://github.com/tpn/winsdk-10/blob/master/Include/10.0.16299.0/winrt/windows.management.deployment.h
    // definition of RuntimeClass_Windows_Management_Deployment_PackageManager is following
    let class_id = "Windows.Management.Deployment.PackageManager\0".encode_utf16().collect::<Vec<u16>>();
    let class_id = WindowsCreateString(&class_id)?;
    // let af: windows::core::IActivationFactory = core::mem::zeroed();

    switch::trace!("indexer", log::Level::Info, "created class_id");

    let af: windows::core::IActivationFactory = RoGetActivationFactory(class_id)?;
    // let pi: windows::core::IInspectable = af.ActivateInstance()?;
    // let pm: windows::Management::Deployment::IPackageManager = pi.into();
    switch::trace!("indexer", log::Level::Info, "created af");

    let pm: windows::Management::Deployment::PackageManager = af.ActivateInstance()?;
    let packages = pm.FindPackages()?;
    return Ok(apps);
}

fn main() -> anyhow::Result<()> {
    switch::log::initialize_test_log(log::Level::Debug, &["indexer"]).unwrap();
    let mut apps = index_exes()?;
    apps.append(unsafe { &mut index_appx()? });
    save_apps(&apps)?;
    return Ok(());
}