use crate::models::ModuleRunnerConfig;
use std::{
	process::Child,
	time::{SystemTime, UNIX_EPOCH},
};

pub struct Process {
	pub process: Option<Child>,
	pub runner_config: ModuleRunnerConfig,
	pub log_dir: Option<String>,
	pub working_dir: String,
	pub last_started_at: u64,
	pub created_at: u64,
	pub start_scheduled_at: Option<u64>,
	pub has_been_crashing: bool,
}

impl Process {
	pub fn new(
		runner_config: ModuleRunnerConfig,
		log_dir: Option<String>,
		working_dir: String,
	) -> Self {
		Process {
			process: None,
			runner_config,
			log_dir,
			working_dir,
			last_started_at: 0,
			created_at: get_current_time(),
			start_scheduled_at: None,
			has_been_crashing: false,
		}
	}

	// returns (is_running, is_crashed)
	pub fn is_process_running(&mut self) -> (bool, bool) {
		if self.process.is_none() {
			return false;
		}

		let process = self.process.as_mut().unwrap();
		match process.try_wait() {
			Ok(Some(status)) => {
				if !status.success() {
					(false, true)
				}
				(false, false)
			} // Process has already exited
			Ok(None) => (true, false),
			Err(error) => (false, true),
		}
	}

	pub async fn respawn(&mut self) {
		// TODO
	}
}

fn get_current_time() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
