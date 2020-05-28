use crate::{
	models::{GuillotineMessage, NodeConfig, RunnerConfig},
	node::{juno_module, module::get_module_from_path, process::Process},
	utils::{constants, logger},
};
use std::{
	collections::HashMap,
	io::Error,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_std::{fs, net::TcpStream, path::Path, sync::Mutex, task};
use future::Either;
use futures::{channel::mpsc::unbounded, future, StreamExt};
use futures_timer::Delay;
use juno::models::{Number, Value};

lazy_static! {
	static ref CLOSE_FLAG: Mutex<bool> = Mutex::new(false);
}

pub async fn run(config: RunnerConfig) {
	if config.name.is_none() {
		logger::error("Node name cannot be null");
		return;
	}

	let log_dir = &config.logs;

	while !try_connecting_to_host(config.node.as_ref().unwrap()).await {
		logger::error(&format!(
			"Could not connect to the host instance of {}. Will try again in {} ms",
			constants::APP_NAME,
			1000
		));
		task::sleep(Duration::from_millis(1000)).await;
		if *CLOSE_FLAG.lock().await {
			return;
		}
	}

	// Populate any auto-start modules here
	let mut auto_start_processes = vec![];
	if config.modules.is_some() {
		let modules_dir = &config.modules.as_ref().unwrap().directory;
		let modules_path = Path::new(modules_dir);

		if modules_path.exists().await && modules_path.is_dir().await {
			let mut dir_iterator = fs::read_dir(modules_dir).await.unwrap();
			while let Some(Ok(dir)) = dir_iterator.next().await {
				if let Some(process) =
					get_module_from_path(dir.path().to_str().unwrap(), log_dir).await
				{
					auto_start_processes.push(process);
				}
			}
		}
	}

	keep_node_alive(config, auto_start_processes).await;
}

pub async fn on_exit() {
	*CLOSE_FLAG.lock().await = true;
}

async fn keep_node_alive(config: RunnerConfig, auto_start_processes: Vec<Process>) {
	// Initialize the guillotine juno module
	let (sender, mut command_receiver) = unbounded::<GuillotineMessage>();
	let response = juno_module::setup_module(
		config.name.as_ref().unwrap().clone(),
		&config.node.unwrap(),
		sender.clone(),
	)
	.await;
	if let Err(error) = response {
		logger::error(&format!("Error setting up Juno module: {}", error));
		return;
	}
	let mut juno_module = response.unwrap();

	let mut ids_to_processes = HashMap::new();

	// First, register all the auto_start_processes
	for mut process in auto_start_processes {
		let response = juno_module::register_module(
			config.name.as_ref().unwrap().clone(),
			&mut juno_module,
			&mut process,
		)
		.await;
		if let Ok(module_id) = response {
			// Then, store their assigned moduleIds in a hashmap.
			ids_to_processes.insert(module_id, process);
		} else {
			logger::error(&format!(
				"Error while registering the module '{}': {}",
				process.runner_config.name,
				response.unwrap_err()
			));
		}
	}

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

				// Check if all the processes are doing okay
				for (module_id, process) in ids_to_processes.iter_mut() {
					if let (false, crashed) = process.is_process_running() {
						// Process ain't running.

						if !process.should_be_running {
							// The process isn't expected to be running.
							// Either it has been stopped or it just isn't started yet
							// Don't bother processing this module. Let it stay dead.
							continue;
						}

						if process.start_scheduled_at.is_some() {
							// The process is already scheduled to start at some point in the future.
							// Don't bother notifying the host about this
							if get_current_time() > process.start_scheduled_at.unwrap() {
								// The current time is more than the time the process is schedued to start at.
								// Start the process and set the scheduled time to none
								process.respawn().await;
								process.start_scheduled_at.take();
							}
							continue;
						}

						// The process just exited. Do something about it
						if crashed {
							process.has_been_crashing = true;
						}

						let response = juno_module
							.call_function(&format!("{}.onProcessExited", constants::APP_NAME), {
								let mut map = HashMap::new();
								map.insert(
									String::from("node"),
									Value::String(config.name.as_ref().unwrap().clone()),
								);
								map.insert(
									String::from("moduleId"),
									Value::Number(Number::PosInt(*module_id)),
								);
								map.insert(String::from("crash"), Value::Bool(crashed));
								map
							})
							.await;
						if let Err(error) = response {
							logger::error(&format!("Error calling the exited function: {}", error));
							continue;
						}
						let response = response.unwrap();

						let mut args = if let Value::Object(args) = response {
							args
						} else {
							logger::error("Response is not an object. Malformed response");
							continue;
						};
						let success = if let Some(Value::Bool(success)) = args.remove("success") {
							success
						} else {
							logger::error(
								"Could not find success key in the response. Malformed object",
							);
							continue;
						};

						if !success {
							logger::error(&if let Some(Value::String(error_msg)) =
								args.remove("error")
							{
								error_msg
							} else {
								String::from("Could not find error key in false success response. Malformed object")
							});
							continue;
						}
						let should_restart = if let Some(Value::Bool(should_restart)) =
							args.remove("shouldRestart")
						{
							should_restart
						} else {
							logger::error("Could not find shouldRestart key in true success response. Malformed object");
							continue;
						};
						let wait_duration = if let Some(Value::Number(wait_duration)) =
							args.remove("waitDuration")
						{
							match wait_duration {
								Number::PosInt(wait_duration) => wait_duration,
								Number::NegInt(wait_duration) => wait_duration as u64,
								Number::Float(wait_duration) => wait_duration as u64,
							}
						} else {
							logger::error("Could not find waitDuration key in true success response. Malformed object");
							continue;
						};

						if should_restart {
							process.start_scheduled_at = Some(get_current_time() + wait_duration);
						}
					// Don't start the process yet. When the respawn is called at the scheduled time,
					// the process will automatically be started
					} else {
						// Process is running
						if process.has_been_crashing
							&& get_current_time() - process.last_started_at > 1000
						{
							// The process has been crashing in the immediate past,
							// and has now been running for more than a second.
							// Notify the host that the process is now running,
							// and remove the crashing flag to stop discriminating against this process
							let response = juno_module
								.call_function(
									&format!("{}.onProcessRunning", constants::APP_NAME),
									{
										let mut map = HashMap::new();
										map.insert(
											String::from("node"),
											Value::String(config.name.as_ref().unwrap().clone()),
										);
										map.insert(
											String::from("moduleId"),
											Value::Number(Number::PosInt(*module_id)),
										);
										map.insert(
											String::from("lastSpawnedAt"),
											Value::Number(Number::PosInt(process.last_started_at)),
										);
										map
									},
								)
								.await;
							if let Err(error) = response {
								logger::error(&format!(
									"Error calling the running function: {}",
									error
								));
								continue;
							}
							let response = response.unwrap();

							let mut args = if let Value::Object(args) = response {
								args
							} else {
								logger::error("Response is not an object. Malformed response");
								continue;
							};
							let success = if let Some(Value::Bool(success)) = args.remove("success")
							{
								success
							} else {
								logger::error(
									"Could not find success key in the response. Malformed object",
								);
								continue;
							};

							if !success {
								logger::error(&if let Some(Value::String(error_msg)) =
									args.remove("error")
								{
									error_msg
								} else {
									String::from("Could not find error key in false success response. Malformed object")
								});
								continue;
							}
							process.has_been_crashing = false;
						}
					}
				}
			}
			Either::Right((command_value, next_timer_future)) => {
				// A command was received. Do something about it
				command_future = command_receiver.next();
				timer_future = next_timer_future;

				if command_value.is_none() {
					// Command is none. Are the senders closed?
					break;
				}

				match command_value.unwrap() {
					GuillotineMessage::RestartProcess {
						module_id,
						response,
					} => {
						if !ids_to_processes.contains_key(&module_id) {
							response.send(Err(String::from("Could not find any process with that moduleId in this runner. Is this stale data?"))).unwrap();
							continue;
						}
						ids_to_processes
							.get_mut(&module_id)
							.unwrap()
							.respawn()
							.await;
						response.send(Ok(())).unwrap();
					}
					GuillotineMessage::AddProcess {
						node_name: _,
						path,
						response,
					} => {
						let mut process = if let Some(process) =
							get_module_from_path(&path, &config.logs).await
						{
							process
						} else {
							response
								.send(Err(format!(
									"{}{}{}",
									"The path provided is not a valid path to a module. ",
									"Please ensure that the path points to a module.json file ",
									"or a folder containing the file"
								)))
								.unwrap();
							continue;
						};

						let result = juno_module::register_module(
							config.name.as_ref().unwrap().clone(),
							&mut juno_module,
							&mut process,
						)
						.await;
						let module_id = if let Ok(module_id) = result {
							module_id
						} else {
							response
								.send(Err(format!(
									"Error while registering the module '{}': {}",
									process.runner_config.name,
									result.unwrap_err()
								)))
								.unwrap();
							continue;
						};

						// Then, store their assigned moduleIds in a hashmap.
						ids_to_processes.insert(module_id, process);
						response.send(Ok(())).unwrap();
					}
					GuillotineMessage::StartProcess {
						module_id,
						response,
					} => {
						if !ids_to_processes.contains_key(&module_id) {
							response.send(Err(String::from("Could not find any process with that moduleId in this runner. Is this stale data?"))).unwrap();
							continue;
						}

						let process = ids_to_processes.get_mut(&module_id).unwrap();
						if !process.is_process_running().0 {
							process.respawn().await;
							process.should_be_running = true;
						}
						response.send(Ok(())).unwrap();
					}
					GuillotineMessage::StopProcess {
						module_id,
						response,
					} => {
						if !ids_to_processes.contains_key(&module_id) {
							response.send(Err(String::from("Could not find any process with that moduleId in this runner. Is this stale data?"))).unwrap();
							continue;
						}

						let process = ids_to_processes.get_mut(&module_id).unwrap();
						if process.is_process_running().0 {
							process.wait_for_quit_or_kill_within(1000).await;
							process.should_be_running = false;
						}
						response.send(Ok(())).unwrap();
					}
					GuillotineMessage::DeleteProcess {
						module_id,
						response,
					} => {
						if !ids_to_processes.contains_key(&module_id) {
							response.send(Err(String::from("Could not find any process with that moduleId in this runner. Is this stale data?"))).unwrap();
							continue;
						}

						let process = ids_to_processes.get_mut(&module_id).unwrap();
						if process.is_process_running().0 {
							process.wait_for_quit_or_kill_within(1000).await;
						}
						ids_to_processes.remove(&module_id);
						response.send(Ok(())).unwrap();
					}
					msg => panic!("Unhandled guillotine message: {:#?}", msg),
				}
			}
		}
	}

	for (_, mut process) in ids_to_processes {
		process.wait_for_quit_or_kill_within(1000).await;
	}
}

async fn try_connecting_to_host(node: &NodeConfig) -> bool {
	if node.connection_type == constants::connection_type::INET_SOCKET {
		let port = node.port.unwrap();
		// Attempt to connect to the port until you can connect
		let port = format!("127.0.0.1:{}", port);
		let mut connection = TcpStream::connect(&port).await;
		if connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(1000)).await;
			connection = TcpStream::connect(&port).await;
			return connection.is_ok();
		}
		true
	} else if node.connection_type == constants::connection_type::UNIX_SOCKET {
		let unix_socket = node.socket_path.clone().unwrap();
		let mut connection = connect_to_unix_socket(&unix_socket).await;
		if connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(1000)).await;
			connection = connect_to_unix_socket(&unix_socket).await;
			return connection.is_ok();
		}
		true
	} else {
		panic!("Connection type is neither unix socket not inet socket. How did you get here?");
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

fn get_current_time() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
