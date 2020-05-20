use crate::models::{ModuleRunnerConfig, ModuleRunningStatus};
use std::time::{SystemTime, UNIX_EPOCH};

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
