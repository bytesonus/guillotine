use std::process::{Child, Command};

pub struct ProcessRunner {
	process: Option<Child>,
	name: String,
	command: String,
	args: Vec<String>,
	envs: Vec<(String, String)>,
}

impl ProcessRunner {
	pub fn new(name: String, command: String) -> Self {
		ProcessRunner {
			process: None,
			name,
			command,
			args: vec![],
			envs: vec![],
		}
	}

	pub fn args(&mut self, args: Vec<String>) -> &mut Self {
		self.args = args;
		self
	}

	pub fn envs(&mut self, envs: Vec<(String, String)>) -> &mut Self {
		self.envs = envs;
		self
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

		let child = Command::new(&self.command)
			.args(&self.args)
			.envs(self.envs.clone())
			.spawn();
		if let Err(err) = child {
			println!("Error spawing child process '{}': {}", self.name, err);
			return;
		}
		self.process = Some(child.unwrap());
	}
}
