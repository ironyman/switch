use std::io::Read;
use serde::{Serialize, Deserialize};

use crate::listcontentprovider::ListContentProvider;
use crate::listcontentprovider::ListItem;

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

enum StartAppsProviderMode {
    StartApps,
    DirectoryListing,
    Url,
}

pub struct StartAppsProvider {
    // User input query string.
    query: String,
    // Apps we read from history db, indexed json files.
    apps: Vec<AppEntry>,
    // If query is a directory path, then this holds the list of
    // directory entry listings corresponding to the queried path.
    directory_listing: Option<Vec<AppEntry>>,
    directory_listing_path: Option<std::path::PathBuf>,
    mode: StartAppsProviderMode,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AppEntryKind {
    // Includes msc, cpl, exes, things you can pass path to ShellExecute to start
    // name in AppEntry is file name and does not include extension nor '.'.
    // Essence is that these are indexed executable files.
    Exe {
        path: String,
        params: String,
    },
    // References to other files, also can be started by passing path to ShellExecute
    // but we want to save the target_path.
    // ShellExecute the link at path, target_path is only for displaying.
    Link {
        path: String,
        params: String,
        target_path: String,
    },
    // UWP/UAP apps, list with get-appxpackage, have to pass shell:AppsFolder\stuff to ShellExecute
    // Path is where the app lives, not used for executing, maybe for displaying.
    Appx {
        identity_id: String,
        publisher_id: String,
        application_id: String,
        path: String,
    },
    // Command string should be parsed into path and params for ShellExecute.
    Command {
        command: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    #[serde(default)]
    pub use_count: u32,
    #[serde(with = "system_time_format")]
    // #[serde(default)]
    #[serde(default = "default_time")]
    pub last_use_time: chrono::DateTime<chrono::Utc>,
    pub kind: AppEntryKind,
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

// https://serde.rs/attr-default.html
// impl Default for chrono::DateTime<chrono::Utc> {
//     fn default() -> Self {
//         // Timeout(30)
//     }
// }
fn default_time() -> chrono::DateTime<chrono::Utc> {
    let epoch = std::time::UNIX_EPOCH.duration_since(std::time::UNIX_EPOCH).unwrap();
    let naive = chrono::NaiveDateTime::from_timestamp(epoch.as_secs() as i64, epoch.subsec_nanos() as u32);
    return chrono::DateTime::from_utc(naive, chrono::Utc);
}

impl ListItem for AppEntry {
    fn as_any(&self) -> &dyn std::any::Any {
        return self;
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        return self;
    }

    fn as_string(&self) -> String {
        let app = self.as_any().downcast_ref::<AppEntry>().unwrap();
        return String::from(app);
    }

    // Used for tab complete. Should return something that matches this entry.
    fn as_matchable_string(&self) -> String {
        let app = self.as_any().downcast_ref::<AppEntry>().unwrap();
        match &app.kind {
            AppEntryKind::Exe { path: _path, ..} => {
                return app.name.clone();
                // return app.name.clone() + " (" + _path + ")";
            },
            AppEntryKind::Link { .. } => {
                return app.name.clone();
            },
            AppEntryKind::Appx { .. } => {
                return app.name.clone();
            },
            AppEntryKind::Command { command } => {
                return command.clone();
            }
        }
        // return app.name.clone();
    }
}

impl AppEntry {
    // Switch usually runs as elevated so that it can set foreground.
    fn start(&self) -> anyhow::Result<()> {
        match &self.kind {
            AppEntryKind::Exe { path, params } 
            | AppEntryKind::Link { path, params, .. }
            => {
                unsafe {
                    let path = (path.clone() + "\0").encode_utf16().collect::<Vec<u16>>();
                    let params = (params.clone() + "\0").encode_utf16().collect::<Vec<u16>>();
                    windows::Win32::UI::Shell::ShellExecuteW(
                        HWND(0),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(path.as_ptr()),
                        PCWSTR(params.as_ptr()),
                        PCWSTR(std::ptr::null()),
                        SW_SHOWNORMAL.0 as i32
                    );
                    return Ok(());
                }
            },
            AppEntryKind::Appx { identity_id, publisher_id, application_id, .. } => {
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
                    return Ok(());
                }
            },
            AppEntryKind::Command { command } => {
                let command = crate::create_process::shell_expand(command);
                let args: Vec<String> = command.split(" ").map(String::from).collect();
                if args.len() < 1 {
                    return Ok(());
                }

                let (path, params) = if command.starts_with("http://") || command.starts_with("https://") {
                    (command.clone(), String::new())
                } else {
                    (args[0].clone(), if args.len() > 1 { args[1..].join(" ") } else { "".to_string() })
                };

                unsafe {
                    let path = (path.clone() + "\0").encode_utf16().collect::<Vec<u16>>();
                    let params = (params.clone() + "\0").encode_utf16().collect::<Vec<u16>>();
                    windows::Win32::UI::Shell::ShellExecuteW(
                        HWND(0),
                        PCWSTR(std::ptr::null()),
                        PCWSTR(path.as_ptr()),
                        PCWSTR(params.as_ptr()),
                        PCWSTR(std::ptr::null()),
                        SW_SHOWNORMAL.0 as i32
                    );
                }
                return Ok(());
            },
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
            // I'm pretty sure that I tried this and it did not work because the new process spawned with the token from the current process and not its thread – 
            ImpersonateLoggedOnUser(new_token);
            let _ = self.start();
            RevertToSelf();

            LocalFree(std::mem::transmute(sid));

            CloseHandle(token);
            CloseHandle(new_token);
        }

        return Ok(());
    }

    fn start_medium(&self) -> anyhow::Result<()> {
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
            match &self.kind {
                AppEntryKind::Exe { path, params } 
                | AppEntryKind::Link { path, params, .. }
                => {
                    let empty = crate::com::Variant::from("".to_owned());
                    let zero = crate::com::Variant::from(SW_SHOWNORMAL.0 as i32);
                    let params = crate::com::Variant::from(params.clone());

                    if let Err(e) = disp_shell.ShellExecute(
                        BSTR::from(path.clone()),
                        &params.0,
                        &empty.0,
                        &empty.0,
                        &zero.0,
                    ) {
                        crate::trace!("start", log::Level::Error, "Start app medium: ShellExecute {:?}", e);
                    }
                },
                AppEntryKind::Appx { identity_id, publisher_id, application_id, .. } => {
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
                },
                AppEntryKind::Command { command } => {
                    let command = crate::create_process::shell_expand(command);
                    let args: Vec<String> = command.split(" ").map(String::from).collect();
                    if args.len() < 1 {
                        return Ok(());
                    }
    
                    let (path, params) = if command.starts_with("http://") || command.starts_with("https://") {
                        (command.clone(), String::new())
                    } else {
                        (args[0].clone(), if args.len() > 1 { args[1..].join(" ") } else { "".to_string() })
                    };

                    crate::trace!("start", log::Level::Error, "Start app medium: command {:?} {:?}", path, params);

                    let empty = crate::com::Variant::from("".to_owned());
                    let zero = crate::com::Variant::from(SW_SHOWNORMAL.0 as i32);
                    let params = crate::com::Variant::from(params.clone());

                    if let Err(e) = disp_shell.ShellExecute(
                        BSTR::from(path.clone()),
                        &params.0,
                        &empty.0,
                        &empty.0,
                        &zero.0,
                    ) {
                        crate::trace!("start", log::Level::Error, "Start app medium: ShellExecute {:?}", e);
                    }
                },
            }

            return Ok(());
        }
    }

    fn exact_match(&self, query: &str) -> bool {
        if self.name.eq(query) {
            return true;
        }

        return false;
    }
}

impl Default for AppEntry {
    fn default() -> Self {
        return AppEntry { 
            name: "".into(),
            use_count: 0,
            last_use_time: chrono::Utc::now(),
            kind: AppEntryKind::Exe {
                path: "".into(),
                params: "".into(),
            }
        };
    }
}

impl From<&AppEntry> for String {
    fn from(app: &AppEntry) -> String {
        match &app.kind {
            AppEntryKind::Exe { path: _path, ..} => {
                return app.name.clone();
                // return app.name.clone() + " (" + _path + ")";
            },
            AppEntryKind::Link { target_path, .. } => {
                return app.name.clone() + " (" + target_path + ")";
            },
            AppEntryKind::Appx { .. } => {
                return app.name.clone();
            },
            AppEntryKind::Command { command } => {
                if app.name.len() > 0 && &app.name != command {
                    return app.name.clone() + " (" + &command + ")";
                } else {
                    return command.clone();
                }
            }
        }
    }
}

impl StartAppsProvider {
    pub fn new() -> Box<Self> {
        // let mut new = Box::new(StartAppsProvider {
        //     apps: vec![],
        //     query: "".into(),
        // });

        // new.fill();
        // return new;

        return Box::new(StartAppsProvider {
            apps: Self::enumerate_start_apps().unwrap(),
            query: String::new(),
            mode: StartAppsProviderMode::StartApps,
            directory_listing: None,
            directory_listing_path: None,
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

    fn open_history_db() -> std::result::Result<rocksdb::DB, rocksdb::Error> {
        // let db = rocksdb::DB::open_default(crate::path::get_app_data_path("history").unwrap()).unwrap();
        // let _ = db.put(&app.name, bincode::serialize(&*app).unwrap());
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        // opts.set_merge_operator("merge history operator", merge_history, merge_history);
        // opts.set_merge_operator_associative("merge history operator", merge_history);
        opts.set_merge_operator_associative("merge history operator", StartAppsProvider::merge_history);
        return rocksdb::DB::open(&opts, crate::path::get_app_data_path("history").unwrap());
    }

    // Order of apps is
    // 1. query
    // 2. history
    // 3. indexed
    fn enumerate_start_apps() -> anyhow::Result<Vec<AppEntry>> {
        // Maybe run indexer if the file is not found. How to safely find indexer.exe?
        // if !std::path::Path::new(&path).exists() {
        // }
        let mut apps: Vec<AppEntry> = vec![Self::create_query_app()];

        // if let Ok(db) = rocksdb::DB::open_default(crate::path::get_app_data_path("history").unwrap()) {
        let mut history_apps: Vec<AppEntry> = vec![];
        if let Ok(db) = Self::open_history_db() {
            for (_key, value) in db.iterator(rocksdb::IteratorMode::Start) {
                history_apps.push(bincode::deserialize(&value).unwrap())
            }
        }

        history_apps = history_apps.into_iter().rev().collect::<Vec<AppEntry>>();
        for a in history_apps.iter() {
            crate::trace!("db", log::Level::Info, "enumerate_start_apps history\n{:?}", a);
        }
        apps.extend(history_apps);

        // Parse all files that matches app*.json
        let root_path = crate::path::get_app_data_path("")?;
        crate::trace!("db", log::Level::Info, "enumerate_start_apps reading root path {:?}", root_path);
        let app_paths = crate::path::get_directory_listing(root_path, "app")?;
        for path in app_paths.iter() {
            if !path.extension().unwrap_or(std::ffi::OsStr::new("")).eq("json") {
                continue;
            }
            crate::trace!("db", log::Level::Info, "enumerate_start_apps reading file {:?}", path);
            let mut file = std::fs::File::open(path)?;
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            apps.append(&mut serde_json::from_str(&buf)?);
        }
        return Ok(apps);
    }

    // This is actually called on read from db so you it doesn't make sense to 
    // set last_use_time in this function...
    fn merge_history(_new_key: &[u8],
                     old_value: Option<&[u8]>,
                     operands: &rocksdb::MergeOperands)
                     -> Option<Vec<u8>> {
        // let ret = old_value
        // .map(|ov| ov.to_vec())
        // .unwrap_or_else(|| vec![]);
        
        // if ret.len() == 0 {
        //     return None;
        // }

        // unsafe {
        //     windows::Win32::System::Diagnostics::Debug::DebugBreak();
        // }

        let mut new_value: Vec<u8> = Vec::with_capacity(operands.len());
        for op in operands {
            new_value.extend_from_slice(op);
            // for e in op {
            //     result.push(*e)
            // }
        }
        
        if let None = old_value {
            // crate::trace!("db", log::Level::Info, "Merge first value on {:?}", std::str::from_utf8(new_key).unwrap());
            return Some(new_value);
            // let mut deserialized: AppEntry = bincode::deserialize(&new_value).ok()?;
            // deserialized.use_count += 1;
            // deserialized.last_use_time = chrono::Utc::now();
            // return bincode::serialize(&deserialized).ok();
        }

        // crate::trace!("db", log::Level::Info, "Merge value on {:?}", std::str::from_utf8(new_key).unwrap());
        // let mut deserialized: AppEntry = bincode::deserialize(old_value.unwrap()).ok()?;
        // deserialized.use_count += 1;
        // deserialized.last_use_time = chrono::Utc::now();

        let mut deserialized: AppEntry = bincode::deserialize(&new_value).ok()?;
        deserialized.use_count += 1;
        // deserialized.last_use_time = chrono::Utc::now();
        return bincode::serialize(&deserialized).ok();
    }

    fn update_history(app: &AppEntry) {
        if let Ok(db) = Self::open_history_db() {
            let mut update_app = app.clone();
            update_app.last_use_time = chrono::Utc::now();
            match db.merge(&app.name, bincode::serialize(&update_app).unwrap()) {
                Err(e) => {
                    crate::trace!("db", log::Level::Error, "Merge history failed: {:?}", e);
                },
                _ => {},
            }
            // let _ = db.flush();
        }
    }

    // fn parse_query_app(app: &mut AppEntry) {
    //     match app.kind {
    //         AppEntryKind::Exe { .. } => {
    //             let args: Vec<String> = app.name.split(" ").map(String::from).collect();
    //             if args.len() < 1 {
    //                 return;
    //             }

    //             app.path = args[0].clone();
    //             app.params = if args.len() > 1 { args[1..].join(" ") } else { "".to_string() };        
    //         },
    //         AppEntryKind::Url {} => {
    //             app.path = app.name.clone();
    //         },
    //         AppEntryKind::DirEntry { .. } => {
    //             app.path = app.name.clone();
    //         },
    //         _ => {}
    //     }
    // }

    fn create_query_app() -> AppEntry {
        AppEntry { 
            name: String::new(),
            kind: AppEntryKind::Command { command: "".into() },
            // Fill in the rest later.
            ..Default::default()
        }
    }

    // fn get_query(&self) -> &str {
    //     return self.query.as_ref();
    //     // return self.apps[0].name.as_ref();
    // }

    // fn get_query_app_mut(&mut self) -> &mut AppEntry {
    //     return &mut self.apps[0];
    // }

    fn should_show_query_app(&self) -> bool {
        let query_words = self.query.split(" ").collect::<Vec<_>>().len();
        if query_words > 1 {
            return true;
        }

        if let StartAppsProviderMode::Url = self.mode {
            return true;
        }

        return false;
    }

    fn query_directory(&mut self) -> Vec<&mut dyn ListItem> {

        let maybe_dir_entry = std::path::Path::new(&self.query);

        let (path, query) = if self.query.ends_with("\\") && maybe_dir_entry.exists() {
            (maybe_dir_entry.to_owned(), String::new())
        } else if maybe_dir_entry.parent().map(|d| d.exists()).unwrap_or(false) {
            (maybe_dir_entry.parent().unwrap().to_owned(), maybe_dir_entry.file_name().unwrap().to_str().unwrap_or("").to_owned())
        } else {
            // panic!("Bad directory, how did we get here? {:?}", self.query);
            // return vec![];
            ("".into(), "".into())
        };

        let query_words = query.split(" ").collect::<Vec<_>>().len();

        let query_app = if query_words > 1 {
            Some(AppEntry {
                name: self.query.clone(),
                kind: AppEntryKind::Command {
                    command: self.query.clone(),
                },
                ..Default::default()
                // ..self.get_query_app().clone()
            })
        } else {
            None
        };

        crate::trace!("query", log::Level::Info, "query_directory query_app: {:?}", &query_app);

        if self.directory_listing_path.is_none() && query_app.is_none() /* || self.directory_listing_path.as_ref().unwrap().to_str() != path.to_str() */ {
            crate::trace!("query", log::Level::Info, "query_directory get_directory_listing: {:?}, {:?}", path, query);

            self.directory_listing_path = Some(path.clone());
            self.directory_listing = Some(crate::path::get_directory_listing(&*path, &*query).unwrap().iter().map(|p| {
                let name = p.to_str().unwrap().to_owned();
                // Checking if directory is really slow over file shares.
                // if p.is_dir() && !name.ends_with("\\") {
                //     name += "\\";
                // }
                return AppEntry {
                    name: name.clone(),
                    kind: AppEntryKind::Exe {
                        path: name,
                        params: "".into(),
                    },
                    ..Default::default()
                };
            }).collect::<Vec<AppEntry>>());
        } else {
            if let Some(app) = query_app {
                self.directory_listing = Some(vec![app]);
            }
        }
        return self.directory_listing.as_mut().unwrap().iter_mut().map(|app| {
            app as &mut dyn ListItem
        }).collect::<Vec<&mut dyn ListItem>>();
    }

    fn clear_directory_listing(&mut self) {
        self.directory_listing = None;
        self.directory_listing_path = None;
    }
}

impl ListContentProvider for StartAppsProvider {
    // type ListItem = AppEntry;

    // // fn query_for_items(&self) -> Vec<&AppEntry> {
    // // Note that the first app in self.apps is the query AppEntry.
    // // Only show query AppeEntry when query_words >= 2 i.e. when user wants to pass cmdline parameters.
    // // If the query cmdline doesn't include parameters then any app the user wants to run should be in search result.
    // // Keep this in sync with query_for_items_mut. 
    // fn query_for_items(&self) -> Vec<&dyn ListItem> {
    //     // self.apps.iter().filter(|&p| {
    //     //     return p.file_name().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().to_lowercase().contains(&self.filter)
    //     // }).collect()
        
    //     if let AppEntryKind::DirEntry { .. } = self.get_query_app().kind {

    //     }

    //     let mut result: Vec<&AppEntry> = self.apps.iter()
    //         .skip(if self.should_show_query_app() { 0 } else { 1 })
    //         .filter(|&app| {
    //         if app.name.to_lowercase().contains(self.get_query()) {
    //             return true;
    //         }

    //         if let AppEntryKind::Link{ target_path, .. } = &app.kind {
    //             if target_path.to_lowercase().contains(self.get_query()) {
    //                 return true;
    //             }
    //         }

    //         return false;
    //     }).collect();
        
    //     if result.len() > 1 && result[1].exact_match(self.get_query())  {
    //         result.remove(0);
    //     }

    //     return result.iter().map(|&app| {
    //         app as &dyn ListItem
    //     }).collect()
    // }
    // Alternatively could probably transmute from & to &mut.
    // https://www.reddit.com/r/rust/comments/afgp9c/is_it_idiomatic_to_write_almost_identical/
    // That means I need a mut version of should_show_query_app?
    fn query_for_items(&mut self) -> Vec<&mut dyn ListItem> {
        // self.apps.iter().filter(|&p| {
        //     return p.file_name().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().to_lowercase().contains(&self.filter)
        // }).collect()

        let const_ref = unsafe { std::mem::transmute::<_, &StartAppsProvider>(self as *mut StartAppsProvider) };
        
        if let StartAppsProviderMode::DirectoryListing = self.mode {
            // crate::trace!("query", log::Level::Info, "query_for_items: query_directory, {:?}", self.get_query_app());
            return self.query_directory();
        }

        crate::trace!("query", log::Level::Info, "query_for_items: should_show_query_app {}", const_ref.should_show_query_app());

        let mut result: Vec<&mut AppEntry> = self.apps.iter_mut()
            // If we have no results, result.len() then we have to show query app, but we dont' know that at this point.
            // And if can't add self.apps[0] (query app) back because this line borrows self.apps.
            // drop(result) also doesn't change the borrow checker status. So we should live this in and remove query app later.
            // .skip(if const_ref.should_show_query_app() { 0 } else { 1 })
            .filter(|app| {
            if app.name.to_lowercase().contains(&self.query.to_lowercase()) {
                return true;
            }

            if let AppEntryKind::Link { target_path, .. } = &app.kind {
                if target_path.to_lowercase().contains(&self.query.to_lowercase()) {
                    return true;
                }
            } else if let AppEntryKind::Command { command } = &app.kind {
                if command.to_lowercase().contains(&self.query.to_lowercase()) {
                    return true;
                }
            }

            return false;
        }).collect();
        /*
        Error on self.apps.iter_mut line, 
        cannot borrow `self.apps` as mutable more than once at a time
        second mutable borrow occurs hererustcE0499
        startappsprovider.rs(644, 76): first mutable borrow occurs here
        startappsprovider.rs(636, 24): let's call the lifetime of this reference `'1`
        startappsprovider.rs(652, 20): returning this value requires that `*self` is borrowed for `'1`
        but I can fix this by putting the first mut borrow, self.get_query_app_mut in its own function o.O
        */

        if (result.len() > 1 && result[1].exact_match(&self.query)) || 
            (result.len() > 1 && !const_ref.should_show_query_app())
        {
            result.remove(0);
        }

        let result = result.into_iter().map(|app| {
            app as &mut dyn ListItem
        }).collect::<Vec<&mut dyn ListItem>>();
        return result;
    }

    fn query_for_names(&mut self) -> Vec<String> {
        self.query_for_items().iter().map(|app| {
            (*app).as_any().downcast_ref::<AppEntry>().unwrap()
        }).map(String::from).collect::<Vec<String>>()
    }

    fn set_query(&mut self, query: String) {
        // TODO: maybe use this?
        // https://stackoverflow.com/questions/34953711/unwrap-inner-type-when-enum-variant-is-known
        // assert!(matches!(self.apps[0].kind, AppEntryKind::Command{..}));
        // if let AppEntryKind::Command { command } = &mut self.apps[0].kind {
        //     *command = query.clone();
        // }
        self.query = query;

        let maybe_dir_entry = std::path::Path::new(&self.query);

        if self.query.len() > 0 &&
            !self.query.ends_with(":") && !self.query.starts_with("%") &&
            (self.query.chars().nth(0).unwrap().is_alphabetic() || self.query.chars().nth(0).unwrap() == '\\')
        {
            // Fall through to remember if we're in DirectoryListing mode.
            crate::trace!("query", log::Level::Info, "set_query AppEntryKind::DirEntry: {}", &self.query);
            if (maybe_dir_entry.is_absolute() && maybe_dir_entry.exists()) || maybe_dir_entry.parent().map(|d| d.exists()).unwrap_or(false) {
                self.clear_directory_listing();
                self.mode = StartAppsProviderMode::DirectoryListing;
            }
        } else if self.query.starts_with("http:") || self.query.starts_with("https:") {
            self.mode = StartAppsProviderMode::Url;
            crate::trace!("query",  log::Level::Info, "set_query AppEntryKind::Url: {}", &self.query);
            // self.apps[0].kind = AppEntryKind::Url { path: self.query.clone() };
        } else {
            self.clear_directory_listing();
            self.mode = StartAppsProviderMode::StartApps;
            crate::trace!("query", log::Level::Info, "set_query AppEntryKind::Command: {}", &self.query);
        }

        if let AppEntryKind::Command { command } = &mut self.apps[0].kind {
            *command = self.query.clone();
            // The name is used as key in history so must be unique.
            self.apps[0].name = self.query.clone();
        } else {
            // self.apps[0].name = self.query.clone();
        }
    }

    fn start(&mut self, filtered_index: usize, elevated: bool) {
        let mut apps = self.query_for_items();
        let app = apps[filtered_index].as_mut_any().downcast_mut::<AppEntry>().unwrap();

        Self::update_history(&*app);

        crate::trace!("start", log::Level::Info, "Start app elevated {:?}: {:?}", elevated, app);

        let start_method = if elevated {
            AppEntry::start
        } else {
            AppEntry::start_medium
        };

        if let Err(e) = start_method(app) {
            crate::trace!("start", log::Level::Info, "Start app error: {:?}", e);
        }
    }

    fn remove(&mut self, filtered_index: usize) {
        let app = {
            let mut apps = self.query_for_items();
            if filtered_index >= apps.len() {
                return;
            }

            apps[filtered_index].as_mut_any().downcast_mut::<AppEntry>().unwrap() as *const AppEntry
        };

        if let Ok(db) = Self::open_history_db() {
            let _ = db.delete(&(unsafe { app.as_ref() }.unwrap()).name);
            let _ = db.flush();
        }

        for i in 0 .. self.apps.len() {
            if &self.apps[i] as *const _ == app as *const _ {
                self.apps.remove(i);
                break;
            }
        }
    }
}