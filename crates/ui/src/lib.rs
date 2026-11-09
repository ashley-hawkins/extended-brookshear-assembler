#![warn(clippy::all, rust_2018_idioms)]
#![feature(const_trait_impl)]
#![feature(const_cmp)]

mod ansi;
mod app;
mod helpers;
pub use app::{App, install_fonts};
