use std::io::Read;
use serde::{Serialize, Deserialize};

use crate::listcontentprovider::ListContentProvider;

use windows::Win32::Foundation::HWND;
use windows::core::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::log::*;

pub struct StartAppsProvider {
    apps: Vec<AppEntry>,
    filter: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AppExecutableInfo {
    // Includes msc, cpl, exes, things you can pass path to ShellExecute to start
    Exe {
        ext: String,
    },
    // Referenes to other files, also can be started by passing path to ShellExecute
    // but we want to save the target_path.
    Link {
        ext: String,
        target_path: String,
    },
    // UWP/UAP apps, list with get-appxpackage, have to pass shell:AppsFolder\stuff to ShellExecute
    Appx {
        identity_id: String,
        publisher_id: String,
        application_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
    pub exe_info: AppExecutableInfo,
}

impl AppEntry {
    fn start(&self) {
        match &self.exe_info {
            AppExecutableInfo::Exe { ext: _ } | AppExecutableInfo::Link { ext: _, target_path: _ } => {
                unsafe {
                    let path = (self.path.to_string() + "\0").encode_utf16().collect::<Vec<u16>>();
                    windows::Win32::UI::Shell::ShellExecuteW(
                        HWND(0),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(path.as_ptr()),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(std::ptr::null()),
                        SW_SHOWNORMAL.0 as i32
                    );
                }
            },
            AppExecutableInfo::Appx { identity_id, publisher_id, application_id } => {
                unsafe {
                    let path = format!("shell:AppsFolder\\{}_{}!{}\0", identity_id, publisher_id, application_id)
                        .encode_utf16().collect::<Vec<u16>>();

                    windows::Win32::UI::Shell::ShellExecuteW(
                        HWND(0),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(path.as_ptr()),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(std::ptr::null()),
                        SW_SHOWNORMAL.0 as i32
                    );
                }
            }
        };
    }
}

impl Default for AppEntry {
    fn default() -> Self {
        return AppEntry { name: "".into(), path: "".into(), exe_info: AppExecutableInfo::Exe { ext: "".into() } };
    }
}

impl StartAppsProvider {
    pub fn new() -> Box<Self> {
        // let mut new = Box::new(StartAppsProvider {
        //     apps: vec![],
        //     filter: "".into(),
        // });

        // new.fill();
        // return new;
        return Box::new(StartAppsProvider {
            apps: Self::enumerate_start_apps().unwrap(),
            filter: "".into(),
        });
    }

    // This takes too long, use indexer to cache apps into apps.json.
    // fn get_user_start(&self) -> std::path::PathBuf {
    //     let user_profile = std::env::var("USERPROFILE").unwrap();
    //     let mut user_start = std::path::PathBuf::new();
    //     user_start.push(std::path::PathBuf::from(user_profile));
    //     user_start.push(std::path::PathBuf::from(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs"));
    //     return user_start
    // }

    // fn add_path_directories(&self, dirs: &mut Vec::<std::path::PathBuf>) {
    //     let path = std::env::var("PATH").unwrap();
    //     let paths = path.split(";");
    //     let paths = paths.map(|p| std::path::PathBuf::from(p))
    //         .filter(|p| p.exists());
    //     dirs.extend(paths);
    // }

    // fn fill(&mut self) {
    //     let mut roots = Vec::<std::path::PathBuf>::new();
    //     roots.push(self.get_user_start());
    //     roots.push(std::path::PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs"));
    //     self.add_path_directories(&mut roots);

    //     for r in roots.iter() {
    //         let results = walkdir::WalkDir::new(r)
    //             .into_iter()
    //             .filter_map(Result::ok)
    //             .map(|f| {
    //                 f.into_path()
    //             })
    //             .filter(|f| f.extension().unwrap_or(std::ffi::OsStr::new("")) == "lnk" || std::ffi::OsStr::new("") == "exe");
    //             // .collect::<Vec<std::path::PathBuf>>();
    //         self.apps.extend(results);
    //     }
    // }

    fn enumerate_start_apps() -> anyhow::Result<Vec<AppEntry>> {
        let path = crate::log::get_app_data_path("apps.json")?;

        // Maybe run indexer if the file is not found. How to safely find indexer.exe?
        // if !std::path::Path::new(&path).exists() {
        // }

        let mut file = std::fs::File::open(path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        let apps: Vec<AppEntry> = serde_json::from_str(&buf)?;
        return Ok(apps);
    }

    fn get_filtered_app_list(&self) -> Vec<&AppEntry> {
        // self.apps.iter().filter(|&p| {
        //     return p.file_name().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().to_lowercase().contains(&self.filter)
        // }).collect()
        return self.apps.iter().filter(|&app| {
            if app.name.to_lowercase().contains(&self.filter) {
                return true;
            }

            if let AppExecutableInfo::Link{ ext: _, target_path } = &app.exe_info {
                if target_path.to_lowercase().contains(&self.filter) {
                    return true;
                }
            }

            return false;
        }).collect();
    }
}

impl ListContentProvider for StartAppsProvider {
    fn get_filtered_list(&self) -> Vec<String> {
        self.get_filtered_app_list().iter().map(|&app| {
            match &app.exe_info {
                AppExecutableInfo::Exe { ext } => {
                    return app.name.clone() + "." + ext;
                },
                AppExecutableInfo::Link { ext, target_path } => {
                    return app.name.clone() + "." + ext + " (" + target_path + ")";
                },
                AppExecutableInfo::Appx { identity_id: _, publisher_id: _, application_id: _ } => {
                    return app.name.clone();
                }
            }
        }).collect::<Vec<String>>()
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    fn activate(&self, filtered_index: usize) {
        let apps = self.get_filtered_app_list();
        if filtered_index >= apps.len() {
            return;
        }
        crate::trace!("activate", log::Level::Info, "Activate app: {:?}", apps[filtered_index]);

        apps[filtered_index].start();
    }
}