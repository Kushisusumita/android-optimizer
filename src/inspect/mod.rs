//! Reading the phone's state: health, hardware, the health score, security and the source of pop-up ads.
//! Nothing here changes the phone.

pub mod adsource;
pub mod compiled;
pub mod diagnostics;
pub mod hardware;
pub mod perms;
pub mod score;
pub mod security;
pub mod speed;
#[cfg(test)]
mod tests;
