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

pub struct StartAppsProvider {
    apps: Vec<AppEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AppExecutableInfo {
    // Includes msc, cpl, exes, things you can pass path to ShellExecute to start
    // name in AppEntry is file name and does not include extension nor '.'.
    // Essence is that these are indexed executable files.
    Exe {
    },
    // References to other files, also can be started by passing path to ShellExecute
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
    DirEntry {
        // A directory entry query has a valid part and a query part i.e.
        // c:\windows\system32\cmd
        // cmd is the query part and the stuff before is the valid part.
        path: std::path::PathBuf,
        query: String,
    },
    Url {
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

impl ListItem for AppEntry {
    fn as_any(&self) -> &dyn std::any::Any {
        return self;
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        return self;
    }
}

impl AppEntry {
    // Switch usually runs as elevated so that it can set foreground.
    fn start(&self) -> anyhow::Result<()> {
        match &self.exe_info {
            AppExecutableInfo::Exe { .. } 
            | AppExecutableInfo::Link { .. }
            | AppExecutableInfo::Url {}
            | AppExecutableInfo::DirEntry { .. }
            => {
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
                    return Ok(());
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
                    return Ok(());
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
                AppExecutableInfo::Exe { .. } 
                | AppExecutableInfo::Link { .. }
                | AppExecutableInfo::Url {}
                | AppExecutableInfo::DirEntry { .. }
                => {
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
        //     AppExecutableInfo::Exe { .. } | AppExecutableInfo::Link { .. } => {
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
            path: "".into(),
            params: "".into(),
            use_count: 0,
            last_use_time: chrono::Utc::now(),
            exe_info: AppExecutableInfo::Exe {}
        };
    }
}

impl From<&AppEntry> for String {
    fn from(app: &AppEntry) -> String {
        match &app.exe_info {
            AppExecutableInfo::Exe { .. } | AppExecutableInfo::Url {} | AppExecutableInfo::DirEntry { .. } => {
                let name = app.name.clone();
                return name;
            },
            AppExecutableInfo::Link { target_path, .. } => {
                return app.name.clone() + " (" + target_path + ")";
            },
            AppExecutableInfo::Appx { .. } => {
                return app.name.clone();
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

    fn enumerate_start_apps() -> anyhow::Result<Vec<AppEntry>> {
        let path = crate::path::get_app_data_path("apps.json")?;

        // Maybe run indexer if the file is not found. How to safely find indexer.exe?
        // if !std::path::Path::new(&path).exists() {
        // }
        let mut apps: Vec<AppEntry> = vec![AppEntry { 
            name: String::new(),
            exe_info: AppExecutableInfo::Exe {},
            // Fill in the rest later.
            ..Default::default()
        }];

        // if let Ok(db) = rocksdb::DB::open_default(crate::path::get_app_data_path("history").unwrap()) {
        let mut history_apps: Vec<AppEntry> = vec![];
        if let Ok(db) = Self::open_history_db() {
            for (_key, value) in db.iterator(rocksdb::IteratorMode::Start) {
                history_apps.push(bincode::deserialize(&value).unwrap())
            }
        }

        history_apps = history_apps.into_iter().rev().collect::<Vec<AppEntry>>();
        apps.extend(history_apps);
        crate::trace!("init", log::Level::Info, "enumerate_start_apps apps {}, {:?}", apps.len(), apps);

        let mut file = std::fs::File::open(path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        apps.append(&mut serde_json::from_str(&buf)?);
        return Ok(apps);
    }

    // TODO: remove this. Not using this anymore.
    // If started with a query that doesn't match a startapp
    // then start as a command.
    fn start_command(&mut self, elevated: bool) -> anyhow::Result<()> {
        let args: Vec<String> = self.get_query().split(" ").map(String::from).collect();
        if args.len() < 1 {
            return Err(anyhow::Error::msg("Missing argument"));
        }

        self.get_query_app_mut().path = args[0].clone();
        self.get_query_app_mut().params = if args.len() > 1 { args[1..].join(" ") } else { "".to_string() };

        crate::trace!("start", log::Level::Error, "Starting command args {:?}, AppEntry {:?}", args, self.get_query_app());

        let db = rocksdb::DB::open_default(crate::path::get_app_data_path("history").unwrap()).unwrap();
        let _ = db.put(self.get_query(), bincode::serialize(self.get_query_app()).unwrap());
    
        if elevated {
            return self.get_query_app().start();
        } else {
            return self.get_query_app().start_medium();
        }
    }

    fn merge_history(new_key: &[u8],
                     old_value: Option<&[u8]>,
                     operands: &rocksdb::MergeOperands)
                     -> Option<Vec<u8>> {
        // let ret = old_value
        // .map(|ov| ov.to_vec())
        // .unwrap_or_else(|| vec![]);
        
        // if ret.len() == 0 {
        //     return None;
        // }

        if let None = old_value {
            let mut result: Vec<u8> = Vec::with_capacity(operands.len());
            for op in operands {
                result.extend_from_slice(op);
                // for e in op {
                //     result.push(*e)
                // }
            }
            crate::trace!("db", log::Level::Info, "Merge first value on {:?}", std::str::from_utf8(new_key).unwrap());
            return Some(result);
        }

        crate::trace!("db", log::Level::Info, "Merge value on {:?}", std::str::from_utf8(new_key).unwrap());
        let mut deserialized: AppEntry = bincode::deserialize(old_value.unwrap()).ok()?;
        deserialized.use_count += 1;
        deserialized.last_use_time = chrono::Utc::now();
        return bincode::serialize(&deserialized).ok();
    }

    fn parse_query_app(app: &mut AppEntry) {
        match app.exe_info {
            AppExecutableInfo::Exe { .. } => {
                let args: Vec<String> = app.name.split(" ").map(String::from).collect();
                if args.len() < 1 {
                    return;
                }

                app.path = args[0].clone();
                app.params = if args.len() > 1 { args[1..].join(" ") } else { "".to_string() };        
            },
            AppExecutableInfo::Url {} => {
                app.path = app.name.clone();
            },
            AppExecutableInfo::DirEntry { .. } => {
                app.path = app.name.clone();
            },
            _ => {}
        }

        if let Ok(db) = Self::open_history_db() {
            match db.merge(&app.name, bincode::serialize(&*app).unwrap()) {
                Err(e) => {
                    crate::trace!("db", log::Level::Error, "Merge history failed: {:?}", e);
                },
                _ => {
                    crate::trace!("db", log::Level::Info, "Merge history succeeded");
                },
            }
            // let _ = db.flush();
        }
    }

    fn get_query(&self) -> &str {
        return self.apps[0].name.as_ref();
    }

    fn get_query_app(&self) -> &AppEntry {
        return &self.apps[0];
    }

    fn get_query_app_mut(&mut self) -> &mut AppEntry {
        return &mut self.apps[0];
    }

    // Keep this in sync with query_for_items. 
    // Alternatively could probably transmute from & to &mut.
    // https://www.reddit.com/r/rust/comments/afgp9c/is_it_idiomatic_to_write_almost_identical/
    // That means I need a mut version of should_show_query_app?
    fn query_for_items_mut(&mut self) -> Vec<&mut dyn ListItem> {
        // self.apps.iter().filter(|&p| {
        //     return p.file_name().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().to_lowercase().contains(&self.filter)
        // }).collect()
        let query = self.get_query().to_string();

        if let AppExecutableInfo::DirEntry { .. } = self.get_query_app().exe_info {

        }

        let const_ref =  unsafe { std::mem::transmute::<_, &StartAppsProvider>(self as *mut StartAppsProvider) };

        let mut result: Vec<&mut AppEntry> = self.apps.iter_mut()
            .skip(if const_ref.should_show_query_app() { 0 } else { 1 })
            .filter(|app| {
            if app.name.to_lowercase().contains(&query) {
                return true;
            }

            if let AppExecutableInfo::Link { target_path, .. } = &app.exe_info {
                if target_path.to_lowercase().contains(&query) {
                    return true;
                }
            }

            return false;
        }).collect();

        if result.len() > 1 && result[1].exact_match(&query)  {
            result.remove(0);
        }

        let result = result.into_iter().map(|app| {
            app as &mut dyn ListItem
        }).collect::<Vec<&mut dyn ListItem>>();
        return result;
    }

    fn should_show_query_app(&self) -> bool {
        let query_words = self.get_query().split(" ").collect::<Vec<_>>().len();
        if query_words > 1 {
            return true;
        }

        if let AppExecutableInfo::Url{} = self.get_query_app().exe_info {
            return true;
        }

        return false;
    }
}

impl ListContentProvider for StartAppsProvider {
    // type ListItem = AppEntry;

    // fn query_for_items(&self) -> Vec<&AppEntry> {
    // Note that the first app in self.apps is the query AppEntry.
    // Only show query AppeEntry when query_words >= 2 i.e. when user wants to pass cmdline parameters.
    // If the query cmdline doesn't include parameters then any app the user wants to run should be in search result.
    // Keep this in sync with query_for_items_mut. 
    fn query_for_items(&self) -> Vec<&dyn ListItem> {
        // self.apps.iter().filter(|&p| {
        //     return p.file_name().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().to_lowercase().contains(&self.filter)
        // }).collect()
        
        if let AppExecutableInfo::DirEntry { .. } = self.get_query_app().exe_info {

        }

        let mut result: Vec<&AppEntry> = self.apps.iter()
            .skip(if self.should_show_query_app() { 0 } else { 1 })
            .filter(|&app| {
            if app.name.to_lowercase().contains(self.get_query()) {
                return true;
            }

            if let AppExecutableInfo::Link{ target_path, .. } = &app.exe_info {
                if target_path.to_lowercase().contains(self.get_query()) {
                    return true;
                }
            }

            return false;
        }).collect();
        
        if result.len() > 1 && result[1].exact_match(self.get_query())  {
            result.remove(0);
        }

        return result.iter().map(|&app| {
            app as &dyn ListItem
        }).collect()
    }

    fn query_for_names(&self) -> Vec<String> {
        self.query_for_items().iter().map(|&app| {
            app.as_any().downcast_ref::<AppEntry>().unwrap()
        }).map(String::from).collect::<Vec<String>>()
    }

    fn set_query(&mut self, query: String) {
        self.apps[0].name = query;

        let maybe_dir_entry = std::path::Path::new(&self.apps[0].name);
        if maybe_dir_entry.exists() {
            self.apps[0].exe_info = AppExecutableInfo::DirEntry { path: maybe_dir_entry.to_owned(), query: "".to_owned() };
        } else if maybe_dir_entry.parent().map(|d| d.exists()).unwrap_or(false) {
            self.apps[0].exe_info = AppExecutableInfo::DirEntry {
                path: maybe_dir_entry.parent().unwrap().to_owned(),
                query: maybe_dir_entry.file_name().unwrap().to_str().unwrap().to_owned(),
            };
        } else if self.apps[0].name.starts_with("http:") || self.apps[0].name.starts_with("https:") {
            self.apps[0].exe_info = AppExecutableInfo::Url {};
        } else {
            self.apps[0].exe_info = AppExecutableInfo::Exe {};
        }
    }

    fn start(&mut self, filtered_index: usize, elevated: bool) {
        let query_app = self.get_query_app() as *const _;
        let mut apps = self.query_for_items_mut();
        let app = apps[filtered_index].as_mut_any().downcast_mut::<AppEntry>().unwrap();

        if app as *const _ == query_app {
            StartAppsProvider::parse_query_app(app);
        }

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
            let mut apps = self.query_for_items_mut();
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