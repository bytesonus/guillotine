use crate::{
	exec::{juno_module, process::ProcessRunner},
	models::{ConfigValue, GuillotineMessage, ModuleRunnerConfig},
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

pub async fn run(config: ConfigValue) {
	let juno_path = config.juno.path.clone();
	let mut pid = 0;

	let mut juno_process = if config.juno.connection_type == "unix_socket" {
		let socket_path = config.juno.socket_path.as_ref().unwrap();
		ProcessRunner::new(
			pid,
			ModuleRunnerConfig::juno_default(
				juno_path,
				vec!["--socket-location".to_string(), socket_path.clone()],
			),
		)
	} else {
		let port = config.juno.port.as_ref().unwrap();
		let bind_addr = config.juno.bind_addr.as_ref().unwrap();

		ProcessRunner::new(
			pid,
			ModuleRunnerConfig::juno_default(
				juno_path,
				vec![
					"--port".to_string(),
					format!("{}", port),
					"--bind-addr".to_string(),
					bind_addr.clone(),
					"-VVV".to_string(),
				],
			),
		)
	};

	let mut tracked_modules = Vec::new();
	let modules_path = Path::new(&config.modules.path);
	if modules_path.exists().await && modules_path.is_dir().await {
		// Get all modules and add them to the list
		let mut dir_iterator = modules_path.read_dir().await.unwrap();
		while let Some(path) = dir_iterator.next().await {
			let mut module = get_module_from_path(pid, path).await;
			if module.is_some() {
				tracked_modules.push(module.take().unwrap());
				pid += 1;
			}
		}
	}

	// Spawn juno before spawing any modules
	while !juno_process.is_process_running() {
		juno_process.respawn();
	}
	ensure_juno_initialized(config.clone()).await;

	keep_processes_alive(juno_process, config, tracked_modules).await;
}

pub async fn on_exit() {
	*CLOSE_FLAG.lock().await = true;
}

async fn get_module_from_path(
	expected_pid: u64,
	path: Result<DirEntry, Error>,
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

	Some(ProcessRunner::new(expected_pid, config.unwrap()))
}

async fn keep_processes_alive(
	mut juno_process: ProcessRunner,
	juno_config: ConfigValue,
	mut processes: Vec<ProcessRunner>,
) {
	// Initialize the guillotine juno module
	let (sender, mut command_receiver) = unbounded::<GuillotineMessage>();
	let _module = juno_module::setup_module(juno_config, sender).await;

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
				while !juno_process.is_process_running() {
					juno_process.respawn();
				}

				for module in processes.iter_mut() {
					// If a module isn't running, respawn it. Simple.
					if !module.is_process_running() {
						module.respawn();
					}
				}
			}
			Either::Right((command_value, next_timer_future)) => {
				// Got a command from juno
				timer_future = next_timer_future;
				command_future = command_receiver.next();

				match command_value {
					Some(cmd) => {
						if let GuillotineMessage::ListProcesses(sender) = cmd {
							let mut runners = vec![juno_process.copy()];
							processes
								.iter()
								.for_each(|process| runners.push(process.copy()));
							sender.send(runners).unwrap();
						}
					}
					None => {
						println!("Got None as a command. Is the sender closed?");
					}
				}
			}
		}
	}

	// Execute exit actions
	// Kill all modules first
	processes.iter_mut().for_each(|module| {
		logger::info(&format!("Quitting process: {}", module.config.name));
		module.send_quit_signal();
	});
	let quit_time = get_current_millis();
	loop {
		// Give the processes some time to die.
		task::sleep(Duration::from_millis(100)).await;

		// If all of the processes are not running, then break
		if processes
			.iter_mut()
			.all(|module| !module.is_process_running())
		{
			break;
		}
		// If some of the processes are running, check if they've been given enough time.
		if get_current_millis() > quit_time + 1000 {
			// They've been trying to quit for more than 1 second. Kill them all and quit
			processes.iter_mut().for_each(|module| {
				logger::info(&format!("Killing process: {}", module.config.name));
				module.kill();
			});
			break;
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

async fn ensure_juno_initialized(config: ConfigValue) {
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
