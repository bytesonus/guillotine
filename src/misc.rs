use crate::process_runner::ProcessRunner;
use clap::{crate_authors, crate_description, crate_name, crate_version};
use futures::channel::oneshot::Sender;

pub const APP_NAME: &str = crate_name!();
pub const APP_VERSION: &str = crate_version!();
pub const APP_AUTHORS: &str = crate_authors!();
pub const APP_ABOUT: &str = crate_description!();

#[allow(dead_code)]
#[derive(Debug)]
pub enum GuillotineMessage {
	ListModules(Sender<Vec<String>>),
	ListProcesses(Sender<Vec<ProcessRunner>>),
	RestartProcess(String),
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
