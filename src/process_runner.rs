use serde_derive::Deserialize;
use std::process::{Child, Command};

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
	config: ModuleConfig,
}

impl ProcessRunner {
	pub fn new(config: ModuleConfig) -> Self {
		ProcessRunner {
			process: None,
			config,
		}
	}

	pub fn is_process_running(&mut self) -> bool {
		if self.process.is_none() {
			return false;
		}

		let process = self.process.as_mut().unwrap();
		match process.try_wait() {
			Ok(Some(_)) => false, // Process has already exited
			Ok(None) => true,
			Err(_) => false,
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
	}
}
