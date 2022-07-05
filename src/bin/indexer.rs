use std::io::Write;

use windows::Win32::System::Environment::*;
use windows::core::*;
use windows::Win32::System::WinRT::*;
// Used implicitly.
// use windows::Management::Deployment::*;
use switch::log::*;
use switch::startappsprovider::{AppEntry, AppExecutableInfo};

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
        path: "%SystemRoot%\\system32\\\0",
        // path2: std::ffi::OsStr::new("%SystemRoot%\\system32\\"),
        // kind: AppKind::Exe,
        max_depth: 0,
    },
    IndexRoot {
        path: "%ProgramData%\\Microsoft\\Windows\\Start Menu\\\0",
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
    let path = switch::log::get_app_data_path("apps.json")?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(serde_json::to_string(&apps)?.as_bytes())?;
    file.sync_all()?;
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
                ext: extension,
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
    let packages = pm.FindPackages()?;
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
                exe_info: AppExecutableInfo::Appx {
                    path: path.to_string_lossy(),
                    identity_id: identity_id.to_string_lossy(),
                    publisher_id: publisher_id.to_string_lossy(),
                    application_id: app_id,
                }
            });
        }
    }
    return Ok(apps);
}

fn main() -> anyhow::Result<()> {
    switch::log::initialize_test_log(log::Level::Debug, &["indexer"]).unwrap();
    let mut apps = index_exes()?;
    apps.append(unsafe { &mut index_appx()? });
    save_apps(&apps)?;
    return Ok(());
}