#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::{MacroSolverApp, WebWorker};

mod config;
mod utils;
mod widgets;
