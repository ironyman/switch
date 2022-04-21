
use crate::windows2::{
    WindowInfo,
    enum_window,
    set_foreground_window_ex,
};


pub trait ListContentProvider {
    fn get_filtered_list(&self) -> Vec<String>;
    fn set_filter(&mut self, filter: String);
    fn activate(&self, filtered_index: usize);
}

pub struct WindowProvider {
    windows: Vec<WindowInfo>,
    filter: String,
}

impl WindowProvider {
    pub fn new() -> Box<Self> {
        Box::new(WindowProvider {
            windows: enum_window().unwrap(),
            filter: "".into(),
        })
    }

    fn get_filtered_window_list(&self) -> Vec<&WindowInfo> {
        if self.windows.len() <= 1 {
            return vec![]
        }
        
        // Skip the first one which is the host of this app, wt or conhost.
        self.windows.iter().skip(1).filter(|&w| {
            if w.image_name.to_lowercase().contains(&self.filter) {
                return true;
            }
            if w.window_text.to_lowercase().contains(&self.filter) {
                return true;
            }
            return false;
        }).collect()
    }
}

impl ListContentProvider for WindowProvider {
    fn get_filtered_list(&self) -> Vec<String> {
        self.get_filtered_window_list().iter().map(|&w| {
            w.to_string()
        }).collect::<Vec<String>>()
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    fn activate(&self, filtered_index: usize) {
        set_foreground_window_ex(self.get_filtered_window_list()[filtered_index].windowh);
    }
}

pub struct StartAppsProvider {
    apps: Vec<std::path::PathBuf>,
    filter: String,
}

impl StartAppsProvider {
    pub fn new() -> Box<Self> {
        let mut new = Box::new(StartAppsProvider {
            apps: vec![],
            filter: "".into(),
        });

        // new.fill();
        new
    }

    fn get_user_start(&self) -> std::path::PathBuf {
        let user_profile = std::env::var("USERPROFILE").unwrap();
        let mut user_start = std::path::PathBuf::new();
        user_start.push(std::path::PathBuf::from(user_profile));
        user_start.push(std::path::PathBuf::from(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs"));
        return user_start
    }

    fn add_path_directories(&self, dirs: &mut Vec::<std::path::PathBuf>) {
        let path = std::env::var("PATH").unwrap();
        let paths = path.split(";");
        let paths = paths.map(|p| std::path::PathBuf::from(p))
            .filter(|p| p.exists());
        dirs.extend(paths);
    }

    fn fill(&mut self) {
        let mut roots = Vec::<std::path::PathBuf>::new();
        roots.push(self.get_user_start());
        roots.push(std::path::PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs"));
        self.add_path_directories(&mut roots);

        for r in roots.iter() {
            let results = walkdir::WalkDir::new(r)
                .into_iter()
                .filter_map(Result::ok)
                .map(|f| {
                    f.into_path()
                })
                .filter(|f| f.extension().unwrap_or(std::ffi::OsStr::new("")) == "lnk" || std::ffi::OsStr::new("") == "exe");
                // .collect::<Vec<std::path::PathBuf>>();
            self.apps.extend(results);
        }
    }

    fn get_app_list(&self) -> Vec<&std::path::PathBuf> {
        self.apps.iter().filter(|&p| {
            return p.file_name().unwrap_or(std::ffi::OsStr::new("")).to_str().unwrap().to_lowercase().contains(&self.filter)
        }).collect()
    }
}


impl ListContentProvider for StartAppsProvider {
    fn get_filtered_list(&self) -> Vec<String> {
        self.get_app_list().iter().map(|w| {
            w.file_name().unwrap().to_str().unwrap().into()
        }).filter(|f: &String| f.to_lowercase().contains(&self.filter))
        .collect::<Vec<String>>()
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    fn activate(&self, filtered_index: usize) {
        // set_foreground_window_ex(self.get_filtered_window_list()[filtered_index].windowh);
    }
}