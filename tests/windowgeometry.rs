use switch::windowgeometry::get_candidate_windows;

use switch::log::*;
use switch::trace;

// cargo test enumerate_windows
// cargo test --package switch --test windowgeometry -- enumerate_windows --exact --nocapture <
// cargo test --test int_test_name -- modname::test_name
#[test]
fn enumerate_windows() {
    let windows = unsafe { get_candidate_windows() };
    switch::log::initialize_test_log(log::Level::Debug, &["directional_switching", "test"]).unwrap();
    trace!("directional_switching", log::Level::Info, "{:?}", windows);
    // println!("{:?}", windows);
}
