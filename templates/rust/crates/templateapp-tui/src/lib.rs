mod about;
mod inbox;
pub mod model;
mod records;
pub mod screen;
mod settings;
pub mod text;
pub mod widgets;

#[cfg(test)]
mod tests;

pub use model::{Model, Notice, Options};
pub use screen::run;
pub use widgets::Key;
