use crate::{
	exec::{juno_module, process::ProcessRunner},
	models::{GuillotineMessage, GuillotineSpecificConfig, ModuleRunnerConfig},
	utils::logger,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_std::{
	fs::{self, DirEntry},
	io::Error,
	net::TcpStream,
	path::Path,
	prelude::*,
	sync::Mutex,
	task,
};
use futures::{
	channel::mpsc::unbounded,
	future::{self, Either},
};
use futures_timer::Delay;

lazy_static! {
	static ref CLOSE_FLAG: Mutex<bool> = Mutex::new(false);
}

pub async fn run(config: GuillotineSpecificConfig) {
	let juno_path = config.juno.path.clone();
	let mut pid = 0;

	let juno_process = if config.juno.connection_type == "unix_socket" {
		let socket_path = config.juno.socket_path.as_ref().unwrap();
		ProcessRunner::new(
			pid,
			ModuleRunnerConfig::juno_default(
				juno_path.clone(),
				vec!["--socket-location".to_string(), socket_path.clone()],
			),
			match &config.modules {
				Some(modules) => {
					if let Some(log_dir) = &modules.logs {
						let main_dir = Path::new(log_dir);
						if !main_dir.exists().await {
							fs::create_dir(&main_dir).await.unwrap();
						}
						let sub_dir = main_dir.join("Juno");
						if !sub_dir.exists().await {
							fs::create_dir(&sub_dir).await.unwrap();
						}
						Some(String::from(sub_dir.to_str().unwrap()))
					} else {
						None
					}
				}
				None => None,
			},
			Path::new(&juno_path)
				.parent()
				.unwrap()
				.to_str()
				.unwrap()
				.to_string(),
		)
	} else {
		let port = config.juno.port.as_ref().unwrap();
		let bind_addr = config.juno.bind_addr.as_ref().unwrap();

		ProcessRunner::new(
			pid,
			ModuleRunnerConfig::juno_default(
				juno_path.clone(),
				vec![
					"--port".to_string(),
					format!("{}", port),
					"--bind-addr".to_string(),
					bind_addr.clone(),
				],
			),
			match &config.modules {
				Some(modules) => {
					if let Some(log_dir) = &modules.logs {
						let main_dir = Path::new(log_dir);
						if !main_dir.exists().await {
							fs::create_dir(&main_dir).await.unwrap();
						}
						let sub_dir = main_dir.join("Juno");
						if !sub_dir.exists().await {
							fs::create_dir(&sub_dir).await.unwrap();
						}
						Some(String::from(sub_dir.to_str().unwrap()))
					} else {
						None
					}
				}
				None => None,
			},
			Path::new(&juno_path)
				.parent()
				.unwrap()
				.to_str()
				.unwrap()
				.to_string(),
		)
	};
	pid += 1;

	let tracked_modules = match &config.modules {
		Some(modules) => {
			let mut tracked_modules = Vec::new();
			let modules_path = Path::new(&modules.path);
			if modules_path.exists().await && modules_path.is_dir().await {
				// Get all modules and add them to the list
				let mut dir_iterator = modules_path.read_dir().await.unwrap();
				while let Some(path) = dir_iterator.next().await {
					if let Some(module) = get_module_from_path(pid, path, &modules.logs).await {
						tracked_modules.push(module);
						pid += 1;
					}
				}
			}
			Some(tracked_modules)
		}
		None => None,
	};

	keep_processes_alive(juno_process, config, tracked_modules).await;
}

pub async fn on_exit() {
	*CLOSE_FLAG.lock().await = true;
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

async fn keep_processes_alive(
	mut juno_process: ProcessRunner,
	juno_config: GuillotineSpecificConfig,
	mut processes: Option<Vec<ProcessRunner>>,
) {
	// Spawn juno before spawing any modules
	while !juno_process.is_process_running() {
		juno_process.respawn().await;
		ensure_juno_initialized(juno_config.clone()).await;
	}
	// Initialize the guillotine juno module
	let (mut sender, mut command_receiver) = unbounded::<GuillotineMessage>();
	let mut module = juno_module::setup_module(juno_config.clone(), sender).await;

	let mut timer_future = Delay::new(Duration::from_millis(100));
	let mut command_future = command_receiver.next();
	loop {
		let selection = future::select(timer_future, command_future).await;
		match selection {
			Either::Left((_, next_command_future)) => {
				if *CLOSE_FLAG.lock().await {
					break;
				}

				// Timer expired
				command_future = next_command_future;
				timer_future = Delay::new(Duration::from_millis(100));

				// Make sure juno is running before checking any other modules
				if !juno_process.is_process_running() {
					module.close().await;
					drop(module);

					juno_process.respawn().await;
					ensure_juno_initialized(juno_config.clone()).await;

					let channel = unbounded::<GuillotineMessage>();
					sender = channel.0;
					command_receiver = channel.1;

					command_future = command_receiver.next();
					module = juno_module::setup_module(juno_config.clone(), sender).await;
				}

				if processes.is_none() {
					continue;
				}
				let processes = processes.as_mut().unwrap();
				for module in processes.iter_mut() {
					// If a module isn't running, respawn it. Simple.
					if !module.is_process_running() {
						module.respawn().await;
					}
				}
			}
			Either::Right((command_value, next_timer_future)) => {
				// Got a command from juno
				timer_future = next_timer_future;
				command_future = command_receiver.next();

				match command_value {
					Some(cmd) => match cmd {
						GuillotineMessage::ListProcesses(sender) => {
							let mut runners = vec![juno_process.copy()];
							if processes.is_some() {
								processes
									.as_ref()
									.unwrap()
									.iter()
									.for_each(|process| runners.push(process.copy()));
							}
							sender.send(runners).unwrap();
						}
						GuillotineMessage::RestartProcess(pid, response_sender) => {
							if processes.is_none() {
								response_sender.send(false).unwrap();
								continue;
							}

							if pid == 0 {
								response_sender.send(true).unwrap();
								module.close().await;
								drop(module);

								juno_process.respawn().await;
								ensure_juno_initialized(juno_config.clone()).await;

								let channel = unbounded::<GuillotineMessage>();
								sender = channel.0;
								command_receiver = channel.1;

								command_future = command_receiver.next();
								module =
									juno_module::setup_module(juno_config.clone(), sender).await;
								continue;
							}

							let module = processes
								.as_mut()
								.unwrap()
								.iter_mut()
								.find(|process| process.module_id == pid);
							if module.is_none() {
								response_sender.send(false).unwrap();
								continue;
							}
							module.unwrap().respawn().await;
							response_sender.send(true).unwrap();
						}
						_ => {}
					},
					None => {
						println!("Got None as a command. Is the sender closed?");
					}
				}
			}
		}
	}

	// Execute exit actions
	// Kill all modules first
	if processes.is_some() {
		processes.as_mut().unwrap().iter_mut().for_each(|module| {
			logger::info(&format!("Quitting process: {}", module.config.name));
			module.send_quit_signal();
		});
		let quit_time = get_current_millis();
		loop {
			// Give the processes some time to die.
			task::sleep(Duration::from_millis(100)).await;
			// If all of the processes are not running, then break
			if processes
				.as_mut()
				.unwrap()
				.iter_mut()
				.all(|module| !module.is_process_running())
			{
				break;
			}
			// If some of the processes are running, check if they've been given enough time.
			if get_current_millis() > quit_time + 1000 {
				// They've been trying to quit for more than 1 second. Kill them all and quit
				processes.unwrap().iter_mut().for_each(|module| {
					logger::info(&format!("Killing process: {}", module.config.name));
					module.kill();
				});
				break;
			}
		}
	}

	// Now quit juno similarly
	logger::info(&format!("Quitting process: {}", juno_process.config.name));
	juno_process.send_quit_signal();
	let quit_time = get_current_millis();
	loop {
		// Give the process some time to die.
		task::sleep(Duration::from_millis(100)).await;

		// If the process is not running, then break
		if !juno_process.is_process_running() {
			break;
		}
		// If the processes is running, check if it's been given enough time.
		if get_current_millis() > quit_time + 1000 {
			// It's been trying to quit for more than 1 second. Kill it and quit
			logger::info(&format!("Killing process: {}", juno_process.config.name));
			juno_process.kill();
			break;
		}
	}
}

async fn ensure_juno_initialized(config: GuillotineSpecificConfig) {
	if config.juno.port.is_some() {
		let port = config.juno.port.unwrap();
		// Keep attempting to connect to the port until you can connect
		let port = format!("127.0.0.1:{}", port);
		let mut connection = TcpStream::connect(&port).await;
		while connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(250)).await;
			connection = TcpStream::connect(&port).await;
		}
	} else {
		let unix_socket = config
			.juno
			.socket_path
			.unwrap_or_else(|| String::from("./juno.sock"));
		let mut connection = connect_to_unix_socket(&unix_socket).await;
		while connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(250)).await;
			connection = connect_to_unix_socket(&unix_socket).await;
		}
	}
}

#[cfg(target_family = "unix")]
async fn connect_to_unix_socket(socket_path: &str) -> Result<(), Error> {
	async_std::os::unix::net::UnixStream::connect(socket_path).await?;
	Ok(())
}
#[cfg(target_family = "windows")]
async fn connect_to_unix_socket(_: &str) -> Result<(), Error> {
	panic!("Unix sockets are not supported on Windows");
}

fn get_current_millis() -> u128 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis()
}
