#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
mod unix;

#[cfg(target_os = "windows")]
pub use self::windows::{cleanup_sockets, init_sockets, make_socket};

#[cfg(not(target_os = "windows"))]
pub use self::unix::{cleanup_sockets, init_sockets, make_socket};