pub trait ListContentProvider {
    fn get_filtered_list(&self) -> Vec<String>;
    fn set_filter(&mut self, filter: String);
    fn start(&mut self, filtered_index: usize);
    fn start_elevated(&mut self, filtered_index: usize);
    fn remove(&mut self, filtered_index: usize);
}
