use crate::{
	parser::ConfigValue,
	process_runner::{ModuleConfig, ProcessRunner},
};
use std::time::Duration;

use async_std::{
	fs::{self, DirEntry},
	io::Error,
	path::Path,
	prelude::*,
	task,
};

pub async fn run(config: ConfigValue) {
	let mut tracked_processes: Vec<ProcessRunner> = Vec::new();

	let gotham_path = config.gotham.path;
	tracked_processes.push(
		if config.gotham.connection_type == "unix_socket" {
			let socket_path = config.gotham.socket_path.unwrap();
			ProcessRunner::new(ModuleConfig::gotham_default(
				gotham_path,
				vec!["--socket-location".to_string(), socket_path],
			))
		} else {
			let port = config.gotham.port.unwrap();
			let bind_addr = config.gotham.bind_addr.unwrap();

			ProcessRunner::new(ModuleConfig::gotham_default(
				gotham_path,
				vec![
					"--port".to_string(),
					format!("{}", port),
					"--bind-addr".to_string(),
					bind_addr,
				],
			))
		},
	);

	let modules_path = Path::new(&config.modules);
	if modules_path.exists().await && modules_path.is_dir().await {
		// Get all modules and add them to the list
		let mut dir_iterator = modules_path.read_dir().await.unwrap();
		while let Some(path) = dir_iterator.next().await {
			let mut module = get_module_from_path(path).await;
			if module.is_some() {
				tracked_processes.push(module.take().unwrap());
			}
		}
	}

	keep_processes_alive(tracked_processes).await;
}

async fn get_module_from_path(path: Result<DirEntry, Error>) -> Option<ProcessRunner> {
	if path.is_err() {
		return None;
	}
	let root_path = path.unwrap().path();
	let module_json = root_path.join("module.json");

	if !module_json.exists().await {
		return None;
	}

	let module_json_contents = fs::read_to_string(module_json).await;
	if module_json_contents.is_err() {
		return None;
	}
	let module_json_contents = module_json_contents.unwrap();

	let config: Result<ModuleConfig, serde_json::Error> =
		serde_json::from_str(&module_json_contents);

	if config.is_err() {
		return None;
	}

	return Some(ProcessRunner::new(config.unwrap()));
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
