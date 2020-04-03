use crate::{parser::ConfigValue, process_runner::ProcessRunner};

use async_std::{path::Path, task};
use std::time::Duration;

pub async fn run(config: ConfigValue) {
	let mut tracked_processes: Vec<ProcessRunner> = Vec::new();

	let gotham_path = config.gotham.path;
	tracked_processes.push(
		if config.gotham.connection_type == "unix_socket" {
			let socket_path = config.gotham.socket_path.unwrap();
			let mut module = ProcessRunner::new("Gotham".to_string(), gotham_path);
			module
				.args(vec!["--socket-location".to_string(), socket_path])
				.envs(vec![]);
			module
		} else {
			let port = config.gotham.port.unwrap();
			let bind_addr = config.gotham.bind_addr.unwrap();

			let mut module = ProcessRunner::new("Gotham".to_string(), gotham_path);
			module
				.args(vec![
					"--port".to_string(),
					format!("{}", port),
					"--bind-addr".to_string(),
					bind_addr,
				])
				.envs(vec![]);
			module
		},
	);

	if Path::new(&config.modules).exists().await {
		// Get all modules and add them to the list
	}

	keep_processes_alive(tracked_processes).await;
}

async fn keep_processes_alive(mut processes: Vec<ProcessRunner>) {
	loop {
		for module in processes.iter_mut() {
			// If a module isn't running, respawn it. Simple.
			if !module.is_process_running() {
				module.respawn();
			}
		}
		task::sleep(Duration::from_millis(100)).await;
	}
}
