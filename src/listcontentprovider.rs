pub trait ListContentProvider {
    fn get_filtered_list(&self) -> Vec<String>;
    fn set_filter(&mut self, filter: String);
    fn activate(&self, filtered_index: usize);
}
