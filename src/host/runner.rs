use crate::{
	host::juno_module,
	models::{
		GuillotineMessage, GuillotineNode, HostConfig, ModuleRunnerConfig, ModuleRunningStatus,
		RunnerConfig, ProcessData,
	},
	node::Process,
	utils::{constants, logger},
};
use std::{
	collections::HashMap,
	io::Error,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_std::{fs, net::TcpStream, path::Path, sync::Mutex, task};
use future::Either;
use futures::{
	channel::{mpsc::unbounded, oneshot::Sender},
	future, StreamExt,
};
use futures_timer::Delay;
use juno::models::{Number, Value};

lazy_static! {
	static ref CLOSE_FLAG: Mutex<bool> = Mutex::new(false);
}

pub async fn run(mut config: RunnerConfig, initialized_sender: Sender<Option<()>>) {
	let host = config.host.take().unwrap();

	if try_connecting_to_juno(&host).await {
		logger::error("An instance of Juno with the same configuration already seems to be running. Duplicate instances are not allowed!");
		initialized_sender.send(None).unwrap();
		return;
	}

	let juno_process = if host.connection_type == constants::connection_type::UNIX_SOCKET {
		let socket_path = host.socket_path.clone().unwrap();
		Process::new(
			ModuleRunnerConfig::juno_default(
				host.path.clone(),
				vec![
					String::from("--socket-location"),
					socket_path,
					String::from("-VVV"),
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
		let port = host.port.unwrap();
		let bind_addr = host.bind_addr.clone().unwrap();
		Process::new(
			ModuleRunnerConfig::juno_default(
				host.path.clone(),
				vec![
					String::from("--port"),
					format!("{}", port),
					String::from("--bind-addr"),
					bind_addr,
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

	keep_host_alive(juno_process, host, initialized_sender).await;
}

pub async fn on_exit() {
	*CLOSE_FLAG.lock().await = true;
}

async fn keep_host_alive(
	mut juno_process: Process,
	juno_config: HostConfig,
	initialized_sender: Sender<Option<()>>,
) {
	// Spawn juno before spawing any modules
	while !juno_process.is_process_running().0 {
		if *CLOSE_FLAG.lock().await {
			logger::info("Exit command received. Exiting");
			initialized_sender.send(None).unwrap();
			return;
		}
		juno_process.respawn().await;
		try_connecting_to_juno(&juno_config).await;
	}

	let mut node_runners = HashMap::new();
	let mut pid = 1;

	// Initialize the guillotine juno module
	let (sender, mut command_receiver) = unbounded::<GuillotineMessage>();
	let mut juno_module = juno_module::setup_host_module(&juno_config, sender.clone()).await;

	logger::info("Host initialized. Waiting for nodes to connect");
	initialized_sender.send(Some(())).unwrap();

	let mut timer_future = Delay::new(Duration::from_millis(100));
	let mut command_future = command_receiver.next();
	loop {
		let selection = future::select(timer_future, command_future).await;

		match selection {
			Either::Left((_, next_command_future)) => {
				if *CLOSE_FLAG.lock().await {
					logger::info("Exit command received. Exiting");
					break;
				}

				// Timer expired
				command_future = next_command_future;
				timer_future = Delay::new(Duration::from_millis(100));

				if juno_process.is_process_running().0 {
					continue;
				}
				// juno_process isn't running. Restart it

				// TODO: when Juno process is restarted, all sub modules should be restarted too
				// But how will you tell the nodes to restart if juno isn't running to tell them to restart?
				// Fuck this shit, don't care (╯°□°）╯︵ ┻━┻

				juno_module.close().await.unwrap();
				drop(juno_module);
				juno_module = juno_module::setup_host_module(&juno_config, sender.clone()).await;
			}
			Either::Right((command_value, next_timer_future)) => {
				// A command was received
				command_future = command_receiver.next();
				timer_future = next_timer_future;

				if command_value.is_none() {
					// Command is none. Are the senders closed?
					break;
				}

				match command_value.unwrap() {
					// Node-host communication stuff
					GuillotineMessage::RegisterNode {
						node_name,
						response,
					} => {
						if !node_runners.contains_key(&node_name) {
							node_runners.insert(
								node_name.clone(),
								GuillotineNode {
									name: node_name,
									processes: vec![],
									connected: true,
								},
							);
						} else {
							node_runners.get_mut(&node_name).unwrap().connected = true;
						}
						response.send(Ok(())).unwrap();
					}
					GuillotineMessage::RegisterProcess {
						node_name,
						process_log_dir,
						process_working_dir,
						process_config,
						process_status,
						process_last_started_at,
						process_created_at,
						response,
					} => {
						if !node_runners.contains_key(&node_name) {
							response.send(Err(format!("Cannot register process. A runner with the name '{}' does not exists", node_name))).unwrap();
							continue;
						}

						// Find a runner which runs a module of the same name
						let runner = node_runners.values_mut().find(|runner| {
							runner
								.get_process_by_name(&process_config.name)
								.is_some()
						});

						let mut process_data = ProcessData::new(
							process_log_dir,
							process_working_dir,
							process_config,
							process_status,
							process_last_started_at,
							process_created_at,
						);

						// There's a runner which runs a module of the same name
						if let Some(runner) = runner {
							// That runner isn't what's registering the process
							if node_name != runner.name {
								response.send(Err(format!("Cannot register process. A process with the name '{}' is already registered under the runner '{}'", process_data.config.name, runner.name))).unwrap();
								continue;
							}
							let process = runner.get_process_by_name(&process_data.config.name);
							// The process isn't offline. This isn't a stale process.
							if process.unwrap().status != ModuleRunningStatus::Offline {
								// The process being registered isn't offline. Duplicate module!
								response.send(Err(format!("Cannot register process. A process with the name '{}' is already registered", process_data.config.name))).unwrap();
								continue;
							}

							// There's a stale module re-registering itself.
							let module_id = process.unwrap().module_id;
							process_data.module_id = module_id;
							runner.register_process(process_data);

							response.send(Ok(module_id)).unwrap();
							continue;
						}

						// So far, no runner runs a process of the same name
						// This is definitely a fresh registration
						// Assign new pid and shit
						let runner = node_runners.get_mut(&node_name);
						if runner.is_none() {
							response.send(Err(format!("Cannot register process. The runner with name '{}' doesn't exist", process_data.config.name))).unwrap();
							continue;
						}
						let runner = runner.unwrap();

						let assigned_pid = pid;
						pid += 1;
						process_data.module_id = assigned_pid;
						runner.register_process(process_data);

						response.send(Ok(assigned_pid)).unwrap();
					}
					GuillotineMessage::ProcessExited {
						node_name,
						module_id,
						crash,
						response,
					} => {
						let runner = node_runners.get_mut(&node_name);
						if runner.is_none() {
							response.send((false, 0)).unwrap();
							continue;
						}
						let runner = runner.unwrap();

						let process = runner.get_process_by_id_mut(module_id);
						if process.is_none() {
							response.send((false, 0)).unwrap();
							continue;
						}
						let process = process.unwrap();

						process.restarts += 1;
						if crash {
							process.crashes += 1;
							process.consequtive_crashes += 1;
							if process.consequtive_crashes > 10 {
								// The process has crashed more than 10 times consequtively.
								// Don't restart the process anymore
								response.send((false, 0)).unwrap();
								process.status = ModuleRunningStatus::Stopped;
							} else {
								// The process has crashed less than 10 times consequtively.
								// Wait for a while and restart the process
								process.last_started_at = get_current_millis();
								response.send((true, 100)).unwrap();
								process.status = ModuleRunningStatus::Running;
							}
						} else {
							process.last_started_at = get_current_millis();
							response.send((true, 0)).unwrap();
							process.status = ModuleRunningStatus::Running;
						}
					}
					GuillotineMessage::ProcessRunning {
						node_name,
						module_id,
						last_spawned_at,
					} => {
						let runner = node_runners.get_mut(&node_name);
						if runner.is_none() {
							continue;
						}
						let runner = runner.unwrap();

						let process = runner.get_process_by_id_mut(module_id);
						if process.is_none() {
							continue;
						}
						let process = process.unwrap();

						process.consequtive_crashes = 0;
						process.last_started_at = last_spawned_at;
						process.status = ModuleRunningStatus::Running;
					}
					GuillotineMessage::NodeDisconnected { node_name } => {
						if let Some(runner) = node_runners.get_mut(&node_name) {
							runner.connected = false;
							runner
								.processes
								.iter_mut()
								.for_each(|process| process.status = ModuleRunningStatus::Offline);
						}
					}

					// Cli stuff
					GuillotineMessage::ListModules { response } => {
						let result = juno_module
							.call_function("juno.listModules", HashMap::new())
							.await;
						if result.is_err() {
							response
								.send(Err(format!(
									"Error listing modules from Juno: {}",
									result.unwrap_err()
								)))
								.unwrap();
							continue;
						}
						let modules = result.unwrap();
						let modules = if let Value::Array(modules) = modules {
							modules
						} else {
							response
								.send(Err(format!("Expected array response. Got {:?}", modules)))
								.unwrap();
							return;
						};
						let modules = modules
							.into_iter()
							.filter_map(|value| {
								if let Value::Object(map) = value {
									Some(map)
								} else {
									None
								}
							})
							.collect();
						response.send(Ok(modules)).unwrap();
					}
					GuillotineMessage::ListNodes { response } => {
						response
							.send(node_runners.iter().map(|(_, node)| node.clone()).collect())
							.unwrap();
					}
					GuillotineMessage::ListAllProcesses { response } => {
						let mut result = vec![];
						node_runners.iter().for_each(|(name, node)| {
							node.processes
								.iter()
								.for_each(|process| result.push((name.clone(), process.clone())));
						});
						response.send(result).unwrap();
					}
					GuillotineMessage::ListProcesses {
						node_name,
						response,
					} => {
						let runner = node_runners.get_mut(&node_name);
						if runner.is_none() {
							response
								.send(Err(format!(
									"Runner node with the name '{}' doesn't exist",
									node_name
								)))
								.unwrap();
							continue;
						}
						let runner = runner.unwrap();

						response.send(Ok(runner.processes.clone())).unwrap();
					}
					GuillotineMessage::RestartProcess {
						module_id,
						response,
					} => {
						let node = node_runners
							.values_mut()
							.find(|node| node.get_process_by_id(module_id).is_some());
						if node.is_none() {
							response
								.send(Err(format!(
									"No node found running the module with the ID {}",
									module_id
								)))
								.unwrap();
							continue;
						}
						let node = node.unwrap();
						if !node.connected {
							response
								.send(Err(format!(
									"The node (with the name '{}') running the module {} is not connected",
									node.name,
									module_id
								)))
								.unwrap();
							continue;
						}

						// Now restart the process
						let result = juno_module
							.call_function(
								&format!(
									"{}-node-{}.respawnProcess",
									constants::APP_NAME,
									node.name
								),
								{
									let mut map = HashMap::new();
									map.insert(
										String::from("moduleId"),
										Value::Number(Number::PosInt(module_id)),
									);
									map
								},
							)
							.await;
						if let Err(error) = result {
							response
								.send(Err(format!("Error sending the restart command: {}", error)))
								.unwrap();
							continue;
						}
						let result = result.unwrap();
						let mut result = if let Value::Object(args) = result {
							args
						} else {
							response
								.send(Err(format!(
									"Response of restart command wasn't an object. Got: {:#?}",
									result
								)))
								.unwrap();
							continue;
						};

						let success = if let Some(Value::Bool(success)) = result.remove("success") {
							success
						} else {
							response
								.send(Err(format!(
									"Success key of restart command wasn't a bool. Got: {:#?}",
									result
								)))
								.unwrap();
							continue;
						};
						if !success {
							response
								.send(Err(
									if let Some(Value::String(error)) = result.remove("error") {
										format!("Error restarting process: {}", error)
									} else {
										format!(
											"Error key of restart command wasn't a string. Got: {:#?}",
											result
										)
									},
								))
								.unwrap();
							continue;
						}
						let process = node.get_process_by_id_mut(module_id);
						if process.is_none() {
							response.send(Err(String::from("Node notified successful process restart, but couldn't find the process in host's memory. Data may be stale"))).unwrap();
							continue;
						}
						let process = process.unwrap();
						process.restarts += 1;
						process.last_started_at = get_current_millis();
						response.send(Ok(())).unwrap();
					}
					msg => panic!("Unhandled guillotine message: {:#?}", msg),
				}
			}
		}
	}

	// TODO: Tell all nodes to quit their processes first

	logger::info(&format!(
		"Quitting process: {}",
		juno_process.runner_config.name
	));
	juno_process.send_quit_signal();
	let quit_time = get_current_millis();
	loop {
		// Give the process some time to die.
		task::sleep(Duration::from_millis(100)).await;

		// If the process is not running, then break
		if !juno_process.is_process_running().0 {
			break;
		}
		// If the processes is running, check if it's been given enough time.
		if get_current_millis() > quit_time + 1000 {
			// It's been trying to quit for more than 1 second. Kill it and quit
			logger::info(&format!(
				"Killing process: {}",
				juno_process.runner_config.name
			));
			juno_process.kill();
			break;
		}
	}
}

async fn try_connecting_to_juno(host: &HostConfig) -> bool {
	if host.connection_type == constants::connection_type::INET_SOCKET {
		let port = host.port.unwrap();
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
	} else if host.connection_type == constants::connection_type::UNIX_SOCKET {
		let unix_socket = host.socket_path.as_ref().unwrap();
		let mut connection = connect_to_unix_socket(unix_socket).await;
		if connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(1000)).await;
			connection = connect_to_unix_socket(unix_socket).await;
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

fn get_current_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards. Wtf?")
		.as_millis() as u64
}
