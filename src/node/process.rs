use crate::{models::ModuleRunnerConfig, utils::logger};
use std::{
	fs::OpenOptions,
	process::{Child, Command, Stdio},
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_std::{path::Path, task};

pub struct Process {
	pub process: Option<Child>,
	pub runner_config: ModuleRunnerConfig,
	pub log_dir: Option<String>,
	pub working_dir: String,
	pub last_started_at: u64,
	pub created_at: u64,
	pub start_scheduled_at: Option<u64>,
	pub has_been_crashing: bool,
	pub should_be_running: bool,
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
			should_be_running: true,
		}
	}

	// returns (is_running, is_crashed)
	pub fn is_process_running(&mut self) -> (bool, bool) {
		if self.process.is_none() {
			return (false, false);
		}

		let process = self.process.as_mut().unwrap();
		match process.try_wait() {
			Ok(Some(status)) => {
				if status.success() {
					(false, false)
				} else {
					(false, true)
				}
			} // Process has already exited
			Ok(None) => (true, false),
			Err(_) => (false, true),
		}
	}

	pub async fn wait_for_quit_or_kill_within(&mut self, wait_ms: u64) {
		if self.process.is_some() && self.is_process_running().0 {
			self.send_quit_signal();
			let quit_time = get_current_time();
			loop {
				// Give the process some time to die.
				task::sleep(Duration::from_millis(100)).await;
				// If the process is not running, then break
				if !self.is_process_running().0 {
					break;
				}
				// If the processes is running, check if it's been given enough time.
				if get_current_time() > quit_time + wait_ms {
					// It's been trying to quit for more than the given duration. Kill it and quit
					logger::info(&format!("Killing process: {}", self.runner_config.name));
					self.kill();
					break;
				}
			}
		}
	}

	pub async fn respawn(&mut self) {
		logger::info(&format!("Respawning '{}'", self.runner_config.name));
		self.wait_for_quit_or_kill_within(1000).await;

		let child = if self.runner_config.interpreter.is_none() {
			let mut command = Command::new(&self.runner_config.command);
			command
				.current_dir(&self.working_dir)
				.args(self.runner_config.args.as_ref().unwrap_or(&vec![]))
				.envs(self.runner_config.envs.as_ref().unwrap_or(&vec![]).clone());

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
						.stdin(Stdio::null())
						.stdout(Stdio::from(output.unwrap()))
						.stderr(Stdio::from(error.unwrap()));
				} else {
					command
						.stdin(Stdio::null())
						.stdout(Stdio::null())
						.stderr(Stdio::null());
				}
			}

			command.spawn()
		} else {
			let mut command = Command::new(self.runner_config.interpreter.as_ref().unwrap());
			command
				.current_dir(&self.working_dir)
				.arg(&self.runner_config.command)
				.args(self.runner_config.args.as_ref().unwrap_or(&vec![]))
				.envs(self.runner_config.envs.as_ref().unwrap_or(&vec![]).clone());

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
						.stdin(Stdio::null())
						.stdout(Stdio::from(output.unwrap()))
						.stderr(Stdio::from(error.unwrap()));
				} else {
					command
						.stdin(Stdio::null())
						.stdout(Stdio::null())
						.stderr(Stdio::null());
				}
			}

			command.spawn()
		};
		if let Err(err) = child {
			logger::error(&format!(
				"Error spawing child process '{}': {}",
				self.runner_config.name, err
			));
			return;
		}
		self.process = Some(child.unwrap());
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
				self.runner_config.name,
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
}

fn get_current_time() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
