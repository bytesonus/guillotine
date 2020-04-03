use crate::parser::ConfigValue;

use async_std::{path::Path, task};
use std::{
	process::{Child, Command},
	time::Duration,
};

pub struct GothamProcess {
	pub process: Option<Child>,
	pub command: String,
	pub args: Vec<String>,
	pub envs: Vec<(String, String)>,
}

pub async fn run(config: ConfigValue) {
	let mut tracked_processes: Vec<GothamProcess> = Vec::new();

	let gotham_path = config.gotham.path;
	tracked_processes.push(if config.gotham.connection_type == "unix_socket" {
		let socket_path = config.gotham.socket_path.unwrap();
		GothamProcess {
			process: None,
			command: gotham_path,
			args: vec!["--socket-location".to_string(), socket_path],
			envs: vec![],
		}
	} else {
		let port = config.gotham.port.unwrap();
		let bind_addr = config.gotham.bind_addr.unwrap();

		GothamProcess {
			process: None,
			command: gotham_path,
			args: vec![
				"--port".to_string(),
				format!("{}", port),
				"--bind-addr".to_string(),
				bind_addr,
			],
			envs: vec![],
		}
	});

	if Path::new(&config.modules).exists().await {
		// Get all modules and add them to the list
	}

	keep_processes_alive(tracked_processes).await;
}

async fn keep_processes_alive(mut processes: Vec<GothamProcess>) {
	loop {
		for module in processes.iter_mut() {
			if module.process.is_none() {
				let child = Command::new(&module.command)
					.args(&module.args)
					.envs(module.envs.clone())
					.spawn();
				if let Err(err) = child {
					println!("Error spawing child process: {}", err);
					continue;
				}
				module.process = Some(child.unwrap());
			}
			let child_process = module.process.as_mut().unwrap();
			match child_process.try_wait() {
				Ok(Some(_)) => {
					// Process has exited. Respawn it next round
					module.process = None;
				},
				Ok(None) => {
					let res = child_process.wait();
					println!("Exited with: {:?}", res);
				}
				Err(e) => println!("Error attempting to wait: {}", e),
			}
		}
		task::sleep(Duration::from_millis(100)).await;
	}
}
