#[test]
fn directory() {
    
    let maybe_dir_entry = std::path::Path::new(r"C:\program files\");

    if maybe_dir_entry.parent().map(|d| d.exists()).unwrap_or(false) {
        // (maybe_dir_entry.parent().unwrap().to_owned(), maybe_dir_entry.file_name().unwrap().to_str().unwrap_or("").to_owned())
        println!("{:?}, {:?}", maybe_dir_entry.parent().unwrap(), maybe_dir_entry.file_name().unwrap());
    }
}
