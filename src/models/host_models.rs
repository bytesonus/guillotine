use super::{ModuleRunnerConfig, ModuleRunningStatus};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct GuillotineNode {
	pub name: String,
	pub processes: Vec<ProcessData>,
	pub connected: bool,
}

impl GuillotineNode {
	pub fn get_process_by_name(&self, name: &str) -> Option<&ProcessData> {
		return self
			.processes
			.iter()
			.find(|process| process.config.name == name);
	}

	pub fn get_process_by_id(&self, id: u64) -> Option<&ProcessData> {
		return self
			.processes
			.iter()
			.find(|process| process.module_id == id);
	}

	pub fn register_process(&mut self, process_data: ProcessData) {
		let position = self
			.processes
			.iter()
			.position(|process| process.config.name == process_data.config.name);
		// If a process with the same name exists, replace the existing value
		if let Some(position) = position {
			// Only update the values that can change. Retain the remaining values
			self.processes[position].log_dir = process_data.log_dir;
			self.processes[position].working_dir = process_data.working_dir;
			self.processes[position].config = process_data.config;
			self.processes[position].status = process_data.status;
		} else {
			self.processes.push(process_data);
		}
	}
}

// Represents a process running under a node
#[derive(Debug, Clone)]
pub struct ProcessData {
	pub log_dir: Option<String>,
	pub working_dir: String,
	pub module_id: u64,
	pub config: ModuleRunnerConfig,
	pub status: ModuleRunningStatus,
	pub restarts: i64,
	pub last_started_at: u64,
	pub crashes: u64,
	pub consequtive_crashes: u64,
	pub created_at: u64,
}

impl ProcessData {
	pub fn new(
		log_dir: Option<String>,
		working_dir: String,
		config: ModuleRunnerConfig,
		status: ModuleRunningStatus,
		last_started_at: u64,
		created_at: u64,
	) -> Self {
		ProcessData {
			log_dir,
			working_dir,
			module_id: 0,
			config,
			status,
			restarts: 0,
			last_started_at,
			crashes: 0,
			consequtive_crashes: 0,
			created_at,
		}
	}

	pub fn get_uptime(&self) -> u64 {
		get_current_time() - self.last_started_at
	}
}

fn get_current_time() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
