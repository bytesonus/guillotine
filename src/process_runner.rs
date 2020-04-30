use serde_derive::Deserialize;
use std::{
	process::{Child, Command},
	time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub enum ModuleRunningStatus {
	Running,
	Offline,
}

#[derive(Deserialize, Debug)]
pub struct ModuleConfig {
	pub name: String,
	pub command: String,
	pub interpreter: Option<String>,
	pub args: Option<Vec<String>>,
	pub envs: Option<Vec<(String, String)>>,
}

impl ModuleConfig {
	pub fn juno_default(path: String, args: Vec<String>) -> Self {
		ModuleConfig {
			name: "Juno".to_string(),
			command: path,
			interpreter: None,
			args: Some(args),
			envs: None,
		}
	}
}

#[derive(Debug)]
pub struct ProcessRunner {
	process: Option<Child>,
	pub module_id: u64,
	pub config: ModuleConfig,
	pub status: ModuleRunningStatus,
	pub restarts: u64,
	pub uptime: u64,
	pub last_started_at: u64,
	pub crashes: u64,
	pub created_at: u64,
}

impl ProcessRunner {
	pub fn new(module_id: u64, config: ModuleConfig) -> Self {
		ProcessRunner {
			module_id,
			process: None,
			config,
			status: ModuleRunningStatus::Offline,
			restarts: 0,
			uptime: 0,
			last_started_at: 0,
			crashes: 0,
			created_at: get_current_time(),
		}
	}

	pub fn is_process_running(&mut self) -> bool {
		if self.process.is_none() {
			return false;
		}

		let process = self.process.as_mut().unwrap();
		match process.try_wait() {
			Ok(Some(status)) => {
				if !status.success() {
					self.crashes += 1;
					self.uptime = 0;
				}
				self.status = ModuleRunningStatus::Offline;
				false
			} // Process has already exited
			Ok(None) => {
				self.status = ModuleRunningStatus::Running;
				self.uptime = get_current_time() - self.last_started_at;
				true
			}
			Err(_) => {
				self.status = ModuleRunningStatus::Offline;
				self.uptime = 0;
				false
			}
		}
	}

	pub fn respawn(&mut self) {
		if self.process.is_some() && self.is_process_running() {
			self.process.as_mut().unwrap().kill().unwrap();
		}

		let child = if self.config.interpreter.is_none() {
			Command::new(&self.config.command)
				.args(self.config.args.as_ref().unwrap_or(&vec![]))
				.envs(self.config.envs.as_ref().unwrap_or(&vec![]).clone())
				.spawn()
		} else {
			Command::new(self.config.interpreter.as_ref().unwrap())
				.arg(&self.config.command)
				.args(self.config.args.as_ref().unwrap_or(&vec![]))
				.envs(self.config.envs.as_ref().unwrap_or(&vec![]).clone())
				.spawn()
		};
		if let Err(err) = child {
			println!(
				"Error spawing child process '{}': {}",
				self.config.name, err
			);
			return;
		}
		self.process = Some(child.unwrap());
		self.restarts += 1;
		self.status = ModuleRunningStatus::Running;
		self.last_started_at = get_current_time();
	}
}

fn get_current_time() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
