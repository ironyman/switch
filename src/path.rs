// use crate::log::*;

pub fn get_installed_exe_path(file: &str) -> String {
    let mut install_path  = std::path::PathBuf::from(std::env::current_exe().unwrap().parent().unwrap());
    install_path.push(file);
    return install_path.into_os_string().into_string().unwrap();
}

pub fn get_app_data_path(file: &str) -> anyhow::Result<String> {
    let app_data = std::env::var("APPDATA")?;
    let dir = app_data  + "\\switch\\";
    let dir = std::path::PathBuf::from(&dir.to_string());

    // just for testing..
    // trace!("path", log::Level::Debug, "HI {}", 1);

    if !dir.exists() {
        std::fs::create_dir_all(&dir)?
    }
    let path = dir.join(file);
    return path.into_os_string().into_string().map_err(|x| anyhow::Error::msg(x.into_string().unwrap()));
    // return Ok(path.to_str().to_owned().unwrap().to_string());
}

// Includes root directory if query is empty.
pub fn get_directory_listing<IntoPath: Into<std::path::PathBuf>, IntoString: Into<String>>(path: IntoPath, query: IntoString) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let root = path.into();
    let dirs = std::fs::read_dir(&root);
    let query = (query.into() as String).to_lowercase();
    // let mut result = Vec::new() as Vec<std::path::PathBuf>;

    let mut result = if query.len() == 0 {
        vec![root]
    } else {
        vec![]
    };

    result.extend(dirs?.filter_map(|d| {
        if let Ok(d) = d {
            if d.path().file_name().unwrap().to_str().unwrap().to_lowercase().contains(&query) {
                return Some(d.path());
            }
        }
        return None;
    }).collect::<Vec<std::path::PathBuf>>());

    return Ok(result);
}