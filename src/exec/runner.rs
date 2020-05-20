use crate::{
	exec::process::ProcessRunner,
	models::{ModuleRunnerConfig, RunnerConfig},
	utils::constants,
};

use async_std::{
	fs::{self, DirEntry},
	io::Error,
	path::Path,
	prelude::*,
};

pub async fn run(config: RunnerConfig) {
	if config.logs.is_some() {
		let log_dir = config.logs.as_ref().unwrap();
		let main_dir = Path::new(log_dir);
		if !main_dir.exists().await {
			fs::create_dir(&main_dir).await.unwrap();
		}
	}

	if config.host.is_some() {
		let host = config.host.unwrap();
		let mut pid = 0;

		let juno_process = if host.connection_type == constants::connection_type::UNIX_SOCKET {
			let socket_path = host.socket_path.as_ref().unwrap();
			ProcessRunner::new(
				pid,
				ModuleRunnerConfig::juno_default(
					host.path.clone(),
					vec![
						String::from("--socket-location"),
						host.socket_path.as_ref().unwrap().clone(),
					],
				),
				match &config.logs {
					Some(log_dir) => {
						let sub_dir = Path::new(log_dir).join("Juno");
						if !sub_dir.exists().await {
							fs::create_dir(&sub_dir).await.unwrap();
						}
						Some(String::from(sub_dir.to_str().unwrap()))
					}
					None => None,
				},
				Path::new(&host.path)
					.parent()
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			)
		} else {
			let port = host.port.as_ref().unwrap();
			let bind_addr = host.bind_addr.as_ref().unwrap();
			ProcessRunner::new(
				pid,
				ModuleRunnerConfig::juno_default(
					host.path.clone(),
					vec![
						"--port".to_string(),
						format!("{}", port),
						"--bind-addr".to_string(),
						bind_addr.clone(),
					],
				),
				match &config.logs {
					Some(log_dir) => {
						let sub_dir = Path::new(log_dir).join("Juno");
						if !sub_dir.exists().await {
							fs::create_dir(&sub_dir).await.unwrap();
						}
						Some(String::from(sub_dir.to_str().unwrap()))
					}
					None => None,
				},
				Path::new(&host.path)
					.parent()
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			)
		};
		pid += 1;

		host::run(juno_process, config.host.clone().unwrap()).await;
	} else if config.node.is_some() {
	} else {
		return;
	}
}

pub async fn on_exit() {
	host::on_exit().await;
}

async fn get_all_available_modules(starting_pid: u64, config: &RunnerConfig) -> Vec<ProcessRunner> {
	if config.modules.is_none() {
		return vec![];
	}
	let modules = config.modules.as_ref().unwrap();

	let mut tracked_modules = vec![];
	let mut pid = starting_pid;

	let modules_path = Path::new(&modules.directory);
	if modules_path.exists().await && modules_path.is_dir().await {
		// Get all modules and add them to the list
		let mut dir_iterator = modules_path.read_dir().await.unwrap();
		while let Some(path) = dir_iterator.next().await {
			if let Some(module) = get_module_from_path(pid, path, &config.logs).await {
				tracked_modules.push(module);
				pid += 1;
			}
		}
	}

	tracked_modules
}

async fn get_module_from_path(
	expected_pid: u64,
	path: Result<DirEntry, Error>,
	log_dir: &Option<String>,
) -> Option<ProcessRunner> {
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

	let config: Result<ModuleRunnerConfig, serde_json::Error> =
		serde_json::from_str(&module_json_contents);

	if config.is_err() {
		return None;
	}
	let config = config.unwrap();

	let runner = if let Some(log_dir) = log_dir {
		let main_dir = Path::new(log_dir);
		if !main_dir.exists().await {
			fs::create_dir(&main_dir).await.unwrap();
		}

		let sub_dir = main_dir.join(&config.name);
		if !sub_dir.exists().await {
			fs::create_dir(&sub_dir).await.unwrap();
		}
		ProcessRunner::new(
			expected_pid,
			config.clone(),
			Some(String::from(sub_dir.to_str().unwrap())),
			root_path.to_str().unwrap().to_string(),
		)
	} else {
		ProcessRunner::new(
			expected_pid,
			config.clone(),
			None,
			root_path.to_str().unwrap().to_string(),
		)
	};

	Some(runner)
}
