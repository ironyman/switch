pub trait ListContentProvider {
    // If I add a type here it would become a type parameter, then how do I put them in an vector, if they all have the same type???
    // Vec<Box<dyn ListContentProvider>>
    // type ListItem;
    // fn query_for_items(&self) -> Vec<&<Self as ListContentProvider>::ListItem>;
    fn query_for_items(&self) -> Vec<&dyn ListItem>;
    fn query_for_names(&self) -> Vec<String>;
    fn set_query(&mut self, filter: String);
    fn start(&mut self, filtered_index: usize);
    fn start_elevated(&mut self, filtered_index: usize);
    fn remove(&mut self, filtered_index: usize);
}

pub trait ListItem /*where Self: Into<String>*/  {
    fn as_any(&self) -> &dyn std::any::Any;
}