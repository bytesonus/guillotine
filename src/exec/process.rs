use crate::{
	logger,
	models::{ModuleRunnerConfig, ModuleRunningStatus},
};
use std::{
	fs::OpenOptions,
	path::Path,
	process::{Child, Command, Stdio},
	time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct ProcessRunner {
	process: Option<Child>,
	pub log_dir: Option<String>,
	pub module_id: u64,
	pub config: ModuleRunnerConfig,
	pub status: ModuleRunningStatus,
	pub restarts: i64,
	pub uptime: u64,
	pub last_started_at: u64,
	pub crashes: u64,
	pub created_at: u64,
}

impl ProcessRunner {
	pub fn new(module_id: u64, config: ModuleRunnerConfig) -> Self {
		ProcessRunner {
			module_id,
			process: None,
			log_dir: None,
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
		logger::info(&format!("Respawning '{}'", self.config.name));

		let child = if self.config.interpreter.is_none() {
			let mut command = Command::new(&self.config.command);
			command
				.args(self.config.args.as_ref().unwrap_or(&vec![]))
				.envs(self.config.envs.as_ref().unwrap_or(&vec![]).clone());

			if self.log_dir.is_some() {
				let log_dir = self.log_dir.as_ref().unwrap();

				let output_location = Path::new(log_dir).join("output.log");
				let error_location = Path::new(log_dir).join("error.log");

				let output = OpenOptions::new()
					.create(true)
					.append(true)
					.open(output_location);
				let error = OpenOptions::new()
					.create(true)
					.append(true)
					.open(error_location);

				if output.is_ok() && error.is_ok() {
					command
						.stdout(Stdio::from(output.unwrap()))
						.stderr(Stdio::from(error.unwrap()));
				}
			}

			command.spawn()
		} else {
			let mut command = Command::new(self.config.interpreter.as_ref().unwrap());
			command
				.arg(&self.config.command)
				.args(self.config.args.as_ref().unwrap_or(&vec![]))
				.envs(self.config.envs.as_ref().unwrap_or(&vec![]).clone());

			if self.log_dir.is_some() {
				let log_dir = self.log_dir.as_ref().unwrap();

				let output_location = Path::new(log_dir).join("output.log");
				let error_location = Path::new(log_dir).join("error.log");

				let output = OpenOptions::new()
					.create(true)
					.append(true)
					.open(output_location);
				let error = OpenOptions::new()
					.create(true)
					.append(true)
					.open(error_location);

				if output.is_ok() && error.is_ok() {
					command
						.stdout(Stdio::from(output.unwrap()))
						.stderr(Stdio::from(error.unwrap()));
				}
			}

			command.spawn()
		};
		if let Err(err) = child {
			logger::error(&format!(
				"Error spawing child process '{}': {}",
				self.config.name, err
			));
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

	pub fn copy(&self) -> Self {
		ProcessRunner {
			module_id: self.module_id,
			process: None,
			log_dir: self.log_dir.clone(),
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
