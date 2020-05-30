use crate::host::{GuillotineNode, ProcessData};
use futures::channel::oneshot::Sender;
use juno::models::Value;
use std::collections::HashMap;

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
		process_data: Box<ProcessData>,
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
		response: Sender<Result<Vec<HashMap<String, Value>>, String>>,
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
	GetProcessInfo {
		module_id: u64,
		response: Sender<Result<(String, ProcessData), String>>, // (node_name, process_data)
	},
	AddProcess {
		node_name: String,
		path: String,
		response: Sender<Result<(), String>>,
	},
	StartProcess {
		module_id: u64,
		response: Sender<Result<(), String>>,
	},
	StopProcess {
		module_id: u64,
		response: Sender<Result<(), String>>,
	},
	DeleteProcess {
		module_id: u64,
		response: Sender<Result<(), String>>,
	},
	GetProcessLogs {
		module_id: u64,
		response: Sender<Result<(String, String), String>>, // (stdout, stderr)
	}
}
// TODO ADD:
// ReloadConfig,
// Save
// Daemonize
// Ping
// SendSignal
