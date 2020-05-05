use crate::logger;

use serde_derive::Deserialize;
use std::{
	process::{Child, Command, Stdio},
	time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
pub enum ModuleRunningStatus {
	Running,
	Offline,
}

#[derive(Deserialize, Debug, Clone)]
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
	pub restarts: i64,
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
			restarts: -1,
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
				.stdin(Stdio::null())
				.stdout(Stdio::null())
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

	#[cfg(target_family = "unix")]
	pub fn send_quit_signal(&mut self) {
		if self.process.is_none() {
			return;
		}
		// Send SIGINT to a process in unix
		use nix::{
			sys::signal::{self, Signal},
			unistd::Pid,
		};

		// send SIGINT to the child
		let result = signal::kill(
			Pid::from_raw(self.process.as_ref().unwrap().id() as i32),
			Signal::SIGINT,
		);
		if result.is_err() {
			logger::error(&format!(
				"Error sending SIGINT to child process '{}': {}",
				self.config.name,
				result.unwrap_err()
			));
		}
	}

	#[cfg(target_family = "windows")]
	pub fn send_quit_signal(&mut self) {
		if self.process.is_none() {
			return;
		}
		// Send ctrl-c event to a process in windows
		// Ref: https://blog.codetitans.pl/post/sending-ctrl-c-signal-to-another-application-on-windows/
		use winapi::um::{
			consoleapi::SetConsoleCtrlHandler,
			wincon::{AttachConsole, FreeConsole, GenerateConsoleCtrlEvent},
		};

		let pid = self.process.as_ref().unwrap().id();
		const CTRL_C_EVENT: u32 = 0;

		unsafe {
			FreeConsole();
			if AttachConsole(pid) > 0 {
				SetConsoleCtrlHandler(None, 1);
				GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0);
			}
		}
	}

	pub fn kill(&mut self) {
		if self.process.is_none() {
			return;
		}
		let result = self.process.as_mut().unwrap().kill();
		if result.is_err() {
			logger::error(&format!("Error killing process: {}", result.unwrap_err()));
		}
	}

	pub fn copy(&self) -> ProcessRunner {
		ProcessRunner {
			module_id: self.module_id,
			process: None,
			config: self.config.clone(),
			status: self.status.clone(),
			restarts: self.restarts,
			uptime: self.uptime,
			last_started_at: self.last_started_at,
			crashes: self.crashes,
			created_at: self.created_at,
		}
	}
}

fn get_current_time() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
