use crate::{
	models::{
		GuillotineMessage,
		GuillotineNode,
		HostConfig,
		ModuleRunnerConfig,
		ModuleRunningStatus,
		ProcessData,
	},
	utils::constants,
};
use std::collections::HashMap;

use async_std::{sync::RwLock, task};
use futures::{
	channel::{mpsc::UnboundedSender, oneshot::channel},
	SinkExt,
};
use juno::{
	models::{Number, Value},
	JunoModule,
};

lazy_static! {
	static ref MESSAGE_SENDER: RwLock<Option<UnboundedSender<GuillotineMessage>>> =
		RwLock::new(None);
}

pub async fn setup_host_module(
	config: &HostConfig,
	sender: UnboundedSender<GuillotineMessage>,
) -> JunoModule {
	MESSAGE_SENDER.write().await.replace(sender);

	let mut module = if config.connection_type == "unix_socket" {
		let socket_path = config.socket_path.as_ref().unwrap();
		JunoModule::from_unix_socket(&socket_path)
	} else {
		let port = config.port.as_ref().unwrap();
		let bind_addr = config.bind_addr.as_ref().unwrap();

		JunoModule::from_inet_socket(&bind_addr, *port)
	};

	module
		.initialize(constants::APP_NAME, constants::APP_VERSION, HashMap::new())
		.await
		.expect(&format!(
			"Could not initialize {} Juno Module",
			constants::APP_NAME
		));

	module
		.declare_function("registerNode", register_node)
		.await
		.unwrap();

	module
		.declare_function("registerProcess", register_process)
		.await
		.unwrap();

	module
		.declare_function("onProcessExited", process_exited)
		.await
		.unwrap();

	module
		.declare_function("onProcessRunning", process_running)
		.await
		.unwrap();

	module
		.declare_function("listModules", list_modules)
		.await
		.unwrap();

	module
		.declare_function("listNodes", list_nodes)
		.await
		.unwrap();

	module
		.declare_function("listAllProcesses", list_all_processes)
		.await
		.unwrap();

	module
		.declare_function("listProcesses", list_processes)
		.await
		.unwrap();

	module
		.declare_function("restartProcess", restart_process)
		.await
		.unwrap();

	module
		.register_hook("juno.moduleDeactivated", module_deactivated)
		.await
		.unwrap();

	module
}

fn register_node(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let name = if let Some(Value::String(value)) = args.remove("name") {
			value
		} else {
			return generate_error_response("Name parameter is not a string");
		};

		let (sender, receiver) = channel::<Result<(), String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::RegisterNode {
				node_name: name,
				response: sender,
			})
			.await;

		let response = receiver.await.unwrap();
		if response.is_ok() {
			Value::Object({
				let mut map = HashMap::new();
				map.insert(String::from("success"), Value::Bool(true));
				map
			})
		} else {
			generate_error_response(&response.unwrap_err())
		}
	})
}

fn register_process(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let node_name = if let Some(Value::String(value)) = args.remove("node") {
			value
		} else {
			return generate_error_response("Name parameter is not a string");
		};

		let log_dir = if let Some(Value::String(dir)) = args.remove("logDir") {
			Some(dir)
		} else {
			None
		};

		let working_dir = if let Some(Value::String(dir)) = args.remove("workingDir") {
			dir
		} else {
			return generate_error_response("Working dir is not a string");
		};

		let mut config = if let Some(Value::Object(config)) = args.remove("config") {
			ModuleRunnerConfig {
				name: if let Some(Value::String(value)) = config.remove("name") {
					value
				} else {
					return generate_error_response("Name is not present in config");
				},
				command: if let Some(Value::String(value)) = config.remove("command") {
					value
				} else {
					return generate_error_response("Command is not present in config");
				},
				interpreter: if let Some(Value::String(value)) = config.remove("interpreter") {
					Some(value)
				} else {
					None
				},
				args: if let Some(Value::Array(value)) = config.remove("args") {
					Some(
						value
							.into_iter()
							.filter_map(|value| {
								if let Value::String(string) = value {
									Some(string)
								} else {
									None
								}
							})
							.collect(),
					)
				} else {
					None
				},
				envs: if let Some(Value::Object(value)) = config.remove("args") {
					Some(
						value
							.into_iter()
							.filter_map(|(key, value)| {
								if let Value::String(string) = value {
									Some((key, string))
								} else {
									None
								}
							})
							.collect(),
					)
				} else {
					None
				},
			}
		} else {
			return generate_error_response("Config is not an object");
		};

		let status = if let Some(Value::String(status)) = args.remove("status") {
			match status.as_ref() {
				"running" => ModuleRunningStatus::Running,
				"stopped" => ModuleRunningStatus::Stopped,
				"offline" => {
					return generate_error_response("Nodes can't declare a module as offline")
				}
				_ => return generate_error_response("Status is not a known value"),
			}
		} else {
			return generate_error_response("Status is not a known value");
		};

		let last_started_at = if let Some(Value::Number(started_at)) = args.remove("lastStartedAt")
		{
			match started_at {
				Number::PosInt(started_at) => started_at,
				Number::NegInt(started_at) => started_at as u64,
				Number::Float(started_at) => started_at as u64,
			}
		} else {
			0
		};

		let created_at = if let Some(Value::Number(created_at)) = args.remove("createdAt") {
			match created_at {
				Number::PosInt(created_at) => created_at,
				Number::NegInt(created_at) => created_at as u64,
				Number::Float(created_at) => created_at as u64,
			}
		} else {
			return generate_error_response("Created at is not a number");
		};

		let (sender, receiver) = channel::<Result<u64, String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::RegisterProcess {
				node_name,
				process_data: ProcessData::new(
					log_dir,
					working_dir,
					config,
					status,
					last_started_at,
					created_at,
				),
				response: sender,
			})
			.await;

		let response = receiver.await.unwrap();
		if let Ok(module_id) = response {
			Value::Object({
				let mut map = HashMap::new();
				map.insert(String::from("success"), Value::Bool(true));
				map.insert(
					String::from("moduleId"),
					Value::Number(Number::PosInt(module_id)),
				);
				map
			})
		} else {
			generate_error_response(&response.unwrap_err())
		}
	})
}

fn process_exited(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let node_name = if let Some(Value::String(value)) = args.remove("node") {
			value
		} else {
			return generate_error_response("Node parameter is not a string");
		};

		let module_id = if let Some(Value::Number(module_id)) = args.remove("moduleId") {
			match module_id {
				Number::PosInt(module_id) => module_id,
				Number::NegInt(module_id) => module_id as u64,
				Number::Float(module_id) => module_id as u64,
			}
		} else {
			return generate_error_response("Module ID is not a number");
		};

		let crash = if let Some(Value::Bool(crash)) = args.remove("crash") {
			crash
		} else {
			return generate_error_response("Module ID is not a number");
		};

		let last_spawned_at =
			if let Some(Value::Number(last_spawned_at)) = args.remove("lastSpawnedAt") {
				match last_spawned_at {
					Number::PosInt(last_spawned_at) => last_spawned_at,
					Number::NegInt(last_spawned_at) => last_spawned_at as u64,
					Number::Float(last_spawned_at) => last_spawned_at as u64,
				}
			} else {
				return generate_error_response("Module ID is not a number");
			};

		let (sender, receiver) = channel::<(bool, u64)>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::ProcessExited {
				node_name,
				module_id,
				crash,
				last_spawned_at,
				response: sender,
			})
			.await;

		let (should_restart, wait_duration_millis) = receiver.await.unwrap();
		Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(true));
			map.insert(String::from("shouldRestart"), Value::Bool(should_restart));
			map.insert(
				String::from("waitDuration"),
				Value::Number(Number::PosInt(wait_duration_millis)),
			);
			map
		})
	})
}

fn process_running(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let node_name = if let Some(Value::String(value)) = args.remove("node") {
			value
		} else {
			return generate_error_response("Node parameter is not a string");
		};

		let module_id = if let Some(Value::Number(module_id)) = args.remove("moduleId") {
			match module_id {
				Number::PosInt(module_id) => module_id,
				Number::NegInt(module_id) => module_id as u64,
				Number::Float(module_id) => module_id as u64,
			}
		} else {
			return generate_error_response("Module ID is not a number");
		};

		let last_spawned_at =
			if let Some(Value::Number(last_spawned_at)) = args.remove("lastSpawnedAt") {
				match last_spawned_at {
					Number::PosInt(last_spawned_at) => last_spawned_at,
					Number::NegInt(last_spawned_at) => last_spawned_at as u64,
					Number::Float(last_spawned_at) => last_spawned_at as u64,
				}
			} else {
				return generate_error_response("Module ID is not a number");
			};

		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::ProcessRunning {
				node_name,
				module_id,
				last_spawned_at,
			})
			.await;

		Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(true));
			map
		})
	})
}

fn module_deactivated(mut data: Value) {
	task::block_on(async {
		let args = if let Value::Object(args) = data {
			args
		} else {
			return;
		};

		let module_id = if let Some(Value::String(module_id)) = args.remove("moduleId") {
			module_id
		} else {
			return;
		};

		if !module_id.starts_with(&format!("{}-node-", constants::APP_NAME)) {
			return;
		}
		let node_name = module_id
			.chars()
			.skip(constants::APP_NAME.len() + "-node-".len())
			.collect();

		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::NodeDisconnected { node_name })
			.await;
	});
}

fn list_modules(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let (sender, receiver) = channel::<Result<Vec<String>, String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::ListModules { response: sender })
			.await;

		let result = receiver.await.unwrap();
		if let Ok(modules) = result {
			Value::Object({
				let mut map = HashMap::new();
				map.insert(String::from("success"), Value::Bool(true));
				map.insert(
					String::from("modules"),
					Value::Array(
						modules
							.into_iter()
							.map(|module| Value::String(module))
							.collect(),
					),
				);
				map
			})
		} else {
			generate_error_response(&result.unwrap_err())
		}
	})
}

fn list_nodes(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let (sender, receiver) = channel::<Vec<GuillotineNode>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::ListNodes { response: sender })
			.await;

		let nodes = receiver.await.unwrap();
		Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(true));
			map.insert(
				String::from("nodes"),
				Value::Array(
					nodes
						.into_iter()
						.map(|node| {
							Value::Object({
								let mut map = HashMap::new();
								map.insert(String::from("name"), Value::String(node.name));
								map.insert(String::from("connected"), Value::Bool(node.connected));
								map.insert(
									String::from("modules"),
									Value::Number(Number::PosInt(node.processes.len() as u64)),
								);
								map
							})
						})
						.collect(),
				),
			);
			map
		})
	})
}

fn list_all_processes(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let (sender, receiver) = channel::<Vec<(String, ProcessData)>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::ListAllProcesses { response: sender })
			.await;

		let processes = receiver.await.unwrap();
		Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(true));
			map.insert(
				String::from("processes"),
				Value::Array(
					processes
						.into_iter()
						.map(|(node, process)| {
							Value::Object({
								let mut map = HashMap::new();

								map.insert(
									String::from("id"),
									Value::Number(Number::PosInt(process.module_id)),
								);
								map.insert(
									String::from("name"),
									Value::String(process.config.name),
								);

								map.insert(
									String::from("logDir"),
									if let Some(log_dir) = process.log_dir {
										Value::String(log_dir)
									} else {
										Value::Null
									},
								);
								map.insert(
									String::from("workingDir"),
									Value::String(process.working_dir),
								);

								map.insert(
									String::from("status"),
									Value::String(String::from(match process.status {
										ModuleRunningStatus::Running => "running",
										ModuleRunningStatus::Stopped => "stopped",
										ModuleRunningStatus::Offline => "offline",
									})),
								);
								map.insert(String::from("node"), Value::String(node));
								map.insert(
									String::from("restarts"),
									Value::Number(Number::NegInt(process.restarts)),
								);
								map.insert(
									String::from("uptime"),
									Value::Number(Number::PosInt(process.get_uptime())),
								);
								map.insert(
									String::from("crashes"),
									Value::Number(Number::PosInt(process.crashes)),
								);
								map.insert(
									String::from("createdAt"),
									Value::Number(Number::PosInt(process.created_at)),
								);

								map
							})
						})
						.collect(),
				),
			);
			map
		})
	})
}

fn list_processes(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let node_name = if let Some(Value::String(value)) = args.remove("node") {
			value
		} else {
			return generate_error_response("Node parameter is not a string");
		};

		let (sender, receiver) = channel::<Result<Vec<ProcessData>, String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::ListProcesses {
				node_name: node_name.clone(),
				response: sender,
			})
			.await;

		let result = receiver.await.unwrap();
		if let Ok(processes) = result {
			Value::Object({
				let mut map = HashMap::new();
				map.insert(String::from("success"), Value::Bool(true));
				map.insert(
					String::from("processes"),
					Value::Array(
						processes
							.into_iter()
							.map(|process| {
								Value::Object({
									let mut map = HashMap::new();

									map.insert(
										String::from("id"),
										Value::Number(Number::PosInt(process.module_id)),
									);
									map.insert(
										String::from("name"),
										Value::String(process.config.name),
									);

									map.insert(
										String::from("logDir"),
										if let Some(log_dir) = process.log_dir {
											Value::String(log_dir)
										} else {
											Value::Null
										},
									);
									map.insert(
										String::from("workingDir"),
										Value::String(process.working_dir),
									);

									map.insert(
										String::from("status"),
										Value::String(String::from(match process.status {
											ModuleRunningStatus::Running => "running",
											ModuleRunningStatus::Stopped => "stopped",
											ModuleRunningStatus::Offline => "offline",
										})),
									);
									map.insert(
										String::from("node"),
										Value::String(node_name.clone()),
									);
									map.insert(
										String::from("restarts"),
										Value::Number(Number::NegInt(process.restarts)),
									);
									map.insert(
										String::from("uptime"),
										Value::Number(Number::PosInt(process.get_uptime())),
									);
									map.insert(
										String::from("crashes"),
										Value::Number(Number::PosInt(process.crashes)),
									);
									map.insert(
										String::from("createdAt"),
										Value::Number(Number::PosInt(process.created_at)),
									);

									map
								})
							})
							.collect(),
					),
				);
				map
			})
		} else {
			generate_error_response(&result.unwrap_err())
		}
	})
}

fn restart_process(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let module_id = if let Some(Value::Number(module_id)) = args.remove("moduleId") {
			match module_id {
				Number::PosInt(module_id) => module_id,
				Number::NegInt(module_id) => module_id as u64,
				Number::Float(module_id) => module_id as u64,
			}
		} else {
			return generate_error_response("Module ID is not a number");
		};

		let (sender, receiver) = channel::<Result<(), String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::RestartProcess {
				module_id,
				response: sender,
			})
			.await;

		let result = receiver.await.unwrap();
		if result.is_ok() {
			Value::Object({
				let mut map = HashMap::new();
				map.insert(String::from("success"), Value::Bool(true));
				map
			})
		} else {
			generate_error_response(&result.unwrap_err())
		}
	})
}

fn generate_error_response(error_message: &str) -> Value {
	Value::Object({
		let mut map = HashMap::new();

		map.insert(String::from("success"), Value::Bool(false));
		if error_message.is_some() {
			map.insert(
				String::from("error"),
				Value::String(String::from(error_message.unwrap())),
			);
		}

		map
	})
}
