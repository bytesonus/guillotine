use clap::{crate_authors, crate_description, crate_name, crate_version};

pub const APP_NAME: &str = crate_name!();
pub const APP_VERSION: &str = crate_version!();
pub const APP_AUTHORS: &str = crate_authors!();
pub const APP_ABOUT: &str = crate_description!();

pub mod connection_type {
	pub const UNIX_SOCKET: &str = "unix_socket";
	pub const INET_SOCKET: &str = "inet_socket";
}
