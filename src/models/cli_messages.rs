use crate::exec::process::ProcessRunner;
use futures::channel::oneshot::Sender;

#[allow(dead_code)]
#[derive(Debug)]
pub enum GuillotineMessage {
	ListModules(Sender<Vec<String>>),
	ListProcesses(Sender<Vec<ProcessRunner>>),
	RestartProcess(u64, Sender<bool>),
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
