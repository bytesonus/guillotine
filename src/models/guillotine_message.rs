use crate::models::{GuillotineNode, ProcessData};
use futures::channel::oneshot::Sender;

#[allow(dead_code)]
#[derive(Debug)]
pub enum GuillotineMessage {
	// Node-host communication stuff
	RegisterNode {
		node_name: String,
		response: Sender<Result<(), String>>,
	},
	RegisterProcess {
		node_name: String,
		process_data: ProcessData,
		response: Sender<Result<u64, String>>,
	},
	ProcessExited {
		node_name: String,
		module_id: u64,
		crash: bool,
		response: Sender<(bool, u64)>, // (should_restart, wait_duration_millis)
	},
	ProcessRunning {
		node_name: String,
		module_id: u64,
		last_spawned_at: u64,
	},
	NodeDisconnected {
		node_name: String,
	},

	// Cli stuff
	ListModules {
		response: Sender<Result<Vec<String>, String>>,
	},
	ListNodes {
		response: Sender<Vec<GuillotineNode>>,
	},
	ListAllProcesses {
		response: Sender<Vec<(String, ProcessData)>>, // (node_name, process_data)
	},
	ListProcesses {
		node_name: String,
		response: Sender<Result<Vec<ProcessData>, String>>,
	},
	RestartProcess {
		module_id: u64,
		response: Sender<Result<(), String>>,
	},
	AddProcess,
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
