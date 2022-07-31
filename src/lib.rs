#![allow(dead_code)]
// #![feature(trait_alias)]

pub use listcontentprovider::ListContentProvider;
pub use windowprovider::WindowProvider;
pub use startappsprovider::StartAppsProvider;

pub mod setforegroundwindow;
pub mod startappsprovider;
pub mod windowprovider;
mod listcontentprovider;
pub mod waitlist;
pub mod console;
pub mod windowgeometry;
pub mod log;
pub mod com;
pub mod create_process;
pub mod path;