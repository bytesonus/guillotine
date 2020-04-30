use clap::{crate_authors, crate_name, crate_version};

pub const APP_NAME: &str = crate_name!();
pub const APP_VERSION: &str = crate_version!();
pub const APP_AUTHORS: &str = crate_authors!();

#[allow(dead_code)]
#[derive(Debug)]
pub enum GuillotineMessage {
	ListModules,
	ListProcesses,
	RestartProcess,
	StopProcess,
	StartProcess,
	DeleteProcess,
	Info,
}
// TODO ADD:
// ReloadConfig,
// Save
// Daemonize
// Ping
// SendSignal
