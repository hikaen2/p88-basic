//! BASIC インタプリタ
pub mod error;
mod file_stmt;
pub mod interp;
mod screen_stmt;
pub mod token;
pub mod value;
#[cfg(test)]
pub mod tests;
