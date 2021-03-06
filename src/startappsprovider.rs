use std::io::Read;
use serde::{Serialize, Deserialize};

use crate::listcontentprovider::ListContentProvider;

use windows::core::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::Threading::*;
use windows::Win32::Foundation::*;
use windows::Win32::Security::*;
use windows::Win32::Security::Authorization::*;
use windows::Win32::System::SystemServices::*;
use windows::Win32::System::Memory::*;

use windows::Win32::System::Com::*;

use windows::Win32::UI::Shell::*;
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
    pub params: String,
    pub use_count: u32,
    #[serde(with = "system_time_format")]
    pub last_use_time: chrono::DateTime<chrono::Utc>,
    pub exe_info: AppExecutableInfo,
}

mod system_time_format {
    pub fn serialize<S>(
        dt: &chrono::DateTime<chrono::Utc>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = serde::Deserialize::deserialize(deserializer)?;
        return match chrono::DateTime::parse_from_rfc3339(s) {
            Ok(dt) => {
                Ok(chrono::DateTime::<chrono::Utc>::from(dt))
            },
            Err(e) => {
                Err(serde::de::Error::custom(e))
            }
        }
    }
}

impl AppEntry {
    // Switch usually runs as elevated so that it can set foreground.
    fn start(&self) {
        match &self.exe_info {
            AppExecutableInfo::Exe { ext: _ } | AppExecutableInfo::Link { ext: _, target_path: _ } => {
                unsafe {
                    let path = (self.path.to_string() + "\0").encode_utf16().collect::<Vec<u16>>();
                    let params = (self.params.to_string() + "\0").encode_utf16().collect::<Vec<u16>>();
                    windows::Win32::UI::Shell::ShellExecuteW(
                        HWND(0),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(path.as_ptr()),
                        PCWSTR(params.as_ptr()),
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

    fn start_medium_impersonate(&self) -> anyhow::Result<()> {
        let mut token = HANDLE(0);

        unsafe {
            if !OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_DUPLICATE | TOKEN_ADJUST_DEFAULT | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
                &mut token).as_bool()
            {
                return Err(anyhow::Error::from(Error::from_win32()));
            }

            let mut new_token = HANDLE(0);
            if !DuplicateTokenEx(token,
                TOKEN_ACCESS_MASK(0),
                std::ptr::null(),
                SecurityImpersonation,
                TokenPrimary,
                &mut new_token).as_bool()
            {
                CloseHandle(token);
                return Err(anyhow::Error::from(Error::from_win32()));
            }

            let mut sid =  PSID::default();
            let medium_integrity = "S-1-16-8192\0".encode_utf16().collect::<Vec<u16>>();
            if !ConvertStringSidToSidW(PCWSTR(medium_integrity.as_ptr()), &mut sid).as_bool() {
                CloseHandle(token);
                CloseHandle(new_token);
                return Err(anyhow::Error::from(Error::from_win32()));
            }

            let mut til = TOKEN_MANDATORY_LABEL::default();
            til.Label.Attributes = SE_GROUP_INTEGRITY as u32;
            til.Label.Sid = sid;

            if !SetTokenInformation(new_token,
                TokenIntegrityLevel,
                std::mem::transmute(&til),
                std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid)).as_bool()
            {
                LocalFree(std::mem::transmute(sid));
                CloseHandle(token);
                CloseHandle(new_token);
                return Err(anyhow::Error::from(Error::from_win32()));
            }

            // https://stackoverflow.com/questions/33594/createprocessasuser-vs-shellexecute
            // I'm pretty sure that I tried this and it did not work because the new process spawned with the token from the current process and not its thread ??? 
            ImpersonateLoggedOnUser(new_token);
            self.start();
            RevertToSelf();

            LocalFree(std::mem::transmute(sid));

            CloseHandle(token);
            CloseHandle(new_token);
        }

        return Ok(());
    }

    // This function has issues where sometimes the created process will not appear in foreground.
    fn start_medium_explorer(&self) -> anyhow::Result<()> {
        unsafe {
            // let mut disp_shell = disp_shell;
            // let disp_shell_owner: Option<IShellDispatch2>;
            // if None == disp_shell {
                CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED).ok();

                let shell = CoCreateInstance::<_, IShellWindows>(
                    &windows::core::GUID::from_u128(0x9BA05972_F6A8_11CF_A442_00A0C90A8F39), // CLSID_ShellWindows
                    None,
                    CLSCTX_ALL).unwrap();

                let pvarloc = crate::com::Variant::from(CSIDL_DESKTOP);
                let pvarlocroot = crate::com::Variant::from(CSIDL_DESKTOP);
                let mut lhwnd = 0i32;
                let mut disp: Option<IDispatch> = None;
                let _ = shell.FindWindowSW(&pvarloc.0 as *const _,
                    &pvarlocroot.0 as *const _,
                    SWC_DESKTOP.0,
                    &mut lhwnd,
                    SWFO_NEEDDISPATCH.0,
                    &mut disp);

                let lhwnd = HWND(lhwnd.try_into().unwrap());
                // let mut window_pid = 0u32;
                let _ = crate::setforegroundwindow::set_foreground_window_terminal(lhwnd);
                // GetWindowThreadProcessId(lhwnd, &mut window_pid);
                // AllowSetForegroundWindow(window_pid);
                // AllowSetForegroundWindow(ASFW_ANY);
                
                // let disp2 = std::mem::transmute::<_, &mut IServiceProvider>(disp.as_mut().unwrap());
                // let mut browser: *mut IShellBrowser = std::ptr::null_mut();
                // let _ = disp2.QueryService(&SID_STopLevelBrowser, &IShellBrowser::IID, std::mem::transmute(&mut browser));
                // let browser: &mut IShellBrowser = browser.as_mut().unwrap();
                let disp2 = disp.as_mut().unwrap().cast::<IServiceProvider>().unwrap(); // probably had to do a cast rather than transmute for disp2

                let mut browser: *mut std::ffi::c_void = std::ptr::null_mut();

                let _ = disp2.QueryService(&SID_STopLevelBrowser, &IShellBrowser::IID, &mut browser);

                let browser: IShellBrowser = std::mem::transmute(browser);

                let view = browser.QueryActiveShellView().unwrap(); // result of FindDesktopFolderView
                // let desktop_folder_view = view.QueryInterface(&IShellView::IID);
                let disp_view: IDispatch = view.GetItemObject(SVGIO_BACKGROUND.0 as u32).unwrap();
                let folder_view = disp_view.cast::<IShellFolderViewDual>().unwrap(); // result of GetDesktopAutomationObject
                
                // disp_shell_owner = Some(folder_view.Application().unwrap().cast::<IShellDispatch2>().unwrap());
                // disp_shell = Some(disp_shell_owner.as_ref().unwrap());
                let disp_shell = folder_view.Application().unwrap().cast::<IShellDispatch2>().unwrap();
            // }
            match &self.exe_info {
                AppExecutableInfo::Exe { ext: _ } | AppExecutableInfo::Link { ext: _, target_path: _ } => {
                    let empty = crate::com::Variant::from("".to_owned());
                    let zero = crate::com::Variant::from(SW_SHOWNORMAL.0 as i32);
                    let params = crate::com::Variant::from(self.params.clone());

                    if let Err(e) = disp_shell.ShellExecute(
                        BSTR::from(self.path.clone()),
                        &params.0,
                        &empty.0,
                        &empty.0,
                        &zero.0,
                    ) {
                        crate::trace!("start", log::Level::Error, "Start app medium: ShellExecute {:?}", e);
                    }

                },
                AppExecutableInfo::Appx { identity_id, publisher_id, application_id } => {
                    let path = format!("shell:AppsFolder\\{}_{}!{}\0", identity_id, publisher_id, application_id);
                    let empty = crate::com::Variant::from("".to_owned());
                    let zero = crate::com::Variant::from(SW_SHOWNORMAL.0 as i32);
                    let _ = disp_shell.ShellExecute(
                        BSTR::from(path.clone()),
                        &empty.0,
                        &empty.0,
                        &empty.0,
                        &zero.0,
                    );

                }
            }

            return Ok(());
        }
    }

    fn start_medium(&self) -> anyhow::Result<()> {
        return self.start_medium_explorer();
        // let shell_cmd = match &self.exe_info {
        //     AppExecutableInfo::Exe { ext: _ } | AppExecutableInfo::Link { ext: _, target_path: _ } => {
        //         self.path.to_owned()
        //     },
        //     AppExecutableInfo::Appx { identity_id, publisher_id, application_id } => {
        //         format!("shell:AppsFolder\\{}_{}!{}\0", identity_id, publisher_id, application_id)
        //     }
        // };

        // unsafe {
        //     let cmdline = crate::create_process::get_installed_exe_path("noconsole.exe") + " --shellexecute " + &shell_cmd;
        //     crate::trace!("start", log::Level::Info, "Start app: create_medium_process {:?}", &cmdline);
        //     let result = crate::create_process::create_medium_process(cmdline);
        //     if let Err(e) = result {
        //         crate::trace!("start", log::Level::Error, "Start app: create_medium_process error {:?}", e);
        //     }
        //     return Ok(());
        // }
    }
}

impl Default for AppEntry {
    fn default() -> Self {
        return AppEntry { 
            name: "".into(),
            path: "".into(),
            params: "".into(),
            use_count: 0,
            last_use_time: chrono::Utc::now(),
            exe_info: AppExecutableInfo::Exe { ext: "".into() }
        };
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
        let path = crate::path::get_app_data_path("apps.json")?;

        // Maybe run indexer if the file is not found. How to safely find indexer.exe?
        // if !std::path::Path::new(&path).exists() {
        // }
        let mut apps: Vec<AppEntry> = vec![];

        if let Ok(db) = rocksdb::DB::open_default(crate::path::get_app_data_path("history.db").unwrap()) {
            for (_key, value) in db.iterator(rocksdb::IteratorMode::Start) {
                apps.push(bincode::deserialize(&value).unwrap())
            }
        }
        
        let mut file = std::fs::File::open(path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        apps.append(&mut serde_json::from_str(&buf)?);
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

    // If started with a filter query that doesn't match a startapp
    // then start as a command.
    fn start_command(&mut self, elevated: bool) {
        let args: Vec<&str> = self.filter.split(" ").collect();
        if args.len() < 1 {
            return;
        }

        crate::trace!("start", log::Level::Error, "Starting command: {:?}", self.filter);

        let cmd = AppEntry {
            name: self.filter.clone(),
            path: args[0].into(),
            params: if args.len() > 1 { args[1..].join(" ") } else { "".to_string() },
            exe_info: AppExecutableInfo::Exe {
                ext: "".into(),
            },
            ..Default::default()
        };

        let db = rocksdb::DB::open_default(crate::path::get_app_data_path("history.db").unwrap()).unwrap();
        let _ = db.put(cmd.name.clone(), bincode::serialize(&cmd).unwrap());
    
        if elevated {
            cmd.start();
        } else {
            let _ = cmd.start_medium();
        }
    }
}

impl ListContentProvider for StartAppsProvider {
    fn get_filtered_list(&self) -> Vec<String> {
        self.get_filtered_app_list().iter().map(|&app| {
            match &app.exe_info {
                AppExecutableInfo::Exe { ext } => {
                    let mut name = app.name.clone();
                    if ext.len() > 0 {
                        name += ".";
                        name += &ext;
                    }
                    return name;
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

    fn start(&mut self, filtered_index: usize) {
        let apps = self.get_filtered_app_list();
        if filtered_index >= apps.len() {
            self.start_command(false);
            return;
        }
        crate::trace!("start", log::Level::Info, "Start app medium: {:?}", apps[filtered_index]);

        if let Err(e) = apps[filtered_index].start_medium() {
            // if let Err(e) = apps[filtered_index].start_medium(Some(&self.disp_shell)) {
            crate::trace!("start", log::Level::Info, "Start app error: {:?}", e);
        }
    }

    fn start_elevated(&mut self, filtered_index: usize) {
        let apps = self.get_filtered_app_list();
        if filtered_index >= apps.len() {
            self.start_command(true);
            return;
        }
        crate::trace!("start", log::Level::Info, "Start app elevated: {:?}", apps[filtered_index]);

        apps[filtered_index].start();
    }

    fn remove(&mut self, _filtered_index: usize) {
    }
}