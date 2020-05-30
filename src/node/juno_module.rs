use crate::{
	models::{GuillotineMessage, NodeConfig},
	node::process::Process,
	utils::constants,
};

use async_std::{sync::RwLock, task};
use futures::{
	channel::{mpsc::UnboundedSender, oneshot::channel},
	SinkExt,
};
use juno::{
	models::{Number, Value},
	JunoModule,
};
use std::collections::HashMap;

lazy_static! {
	static ref MESSAGE_SENDER: RwLock<Option<UnboundedSender<GuillotineMessage>>> =
		RwLock::new(None);
}

pub async fn setup_module(
	node_name: String,
	node: &NodeConfig,
	sender: UnboundedSender<GuillotineMessage>,
) -> Result<JunoModule, String> {
	MESSAGE_SENDER.write().await.replace(sender);

	let mut juno_module = if node.connection_type == constants::connection_type::UNIX_SOCKET {
		JunoModule::from_unix_socket(node.socket_path.as_ref().unwrap())
	} else {
		JunoModule::from_inet_socket(node.ip.as_ref().unwrap(), node.port.unwrap())
	};

	juno_module
		.initialize(
			&format!("{}-node-{}", constants::APP_NAME, node_name),
			constants::APP_VERSION,
			{
				let mut map = HashMap::new();
				map.insert(
					String::from(constants::APP_NAME),
					String::from(constants::APP_VERSION),
				);
				map
			},
		)
		.await
		.unwrap();

	juno_module
		.declare_function("respawnProcess", respawn_process)
		.await
		.unwrap();

	juno_module
		.declare_function("addProcess", add_process)
		.await
		.unwrap();

	juno_module
		.declare_function("startProcess", start_process)
		.await
		.unwrap();

	juno_module
		.declare_function("stopProcess", stop_process)
		.await
		.unwrap();

	juno_module
		.declare_function("deleteProcess", delete_process)
		.await
		.unwrap();

	juno_module
		.declare_function("getLogs", get_logs)
		.await
		.unwrap();

	// Register node here
	let response = juno_module
		.call_function(&format!("{}.registerNode", constants::APP_NAME), {
			let mut map = HashMap::new();
			map.insert(String::from("name"), Value::String(node_name));
			map
		})
		.await
		.unwrap();

	if let Value::Object(mut response) = response {
		if response.remove("success").unwrap() == Value::Bool(true) {
			Ok(juno_module)
		} else if let Some(error) = response.remove("error") {
			Err(if let Value::String(error) = error {
				error
			} else {
				return Err(format!(
					"Expected a string response for error. Got: {:#?}",
					error
				));
			})
		} else {
			Err(format!(
				"Expected a boolean success key in the response. Malformed object: {:#?}",
				response
			))
		}
	} else {
		Err(format!(
			"Expected an object response while registering process. Got: {:#?}",
			response
		))
	}
}

pub async fn register_module(
	node_name: String,
	juno_module: &mut JunoModule,
	process: &mut Process,
) -> Result<u64, String> {
	let mut args = HashMap::new();

	args.insert(String::from("node"), Value::String(node_name));
	if let Some(log_dir) = process.log_dir.clone() {
		args.insert(String::from("logDir"), Value::String(log_dir));
	}
	args.insert(
		String::from("workingDir"),
		Value::String(process.working_dir.clone()),
	);
	args.insert(
		String::from("config"),
		Value::Object({
			let mut map = HashMap::new();
			map.insert(
				String::from("name"),
				Value::String(process.runner_config.name.clone()),
			);
			map.insert(
				String::from("command"),
				Value::String(process.runner_config.command.clone()),
			);
			if let Some(interpreter) = process.runner_config.interpreter.clone() {
				map.insert(String::from("intepreter"), Value::String(interpreter));
			}
			if let Some(args) = process.runner_config.args.clone() {
				map.insert(
					String::from("args"),
					Value::Array(args.into_iter().map(Value::String).collect()),
				);
			}
			if let Some(envs) = process.runner_config.envs.clone() {
				map.insert(
					String::from("args"),
					Value::Object(
						envs.into_iter()
							.map(|(key, value)| (key, Value::String(value)))
							.collect(),
					),
				);
			}
			map
		}),
	);

	args.insert(
		String::from("status"),
		Value::String(String::from(match process.is_process_running() {
			(true, _) => "running",
			(false, _) => "stopped",
		})),
	);
	args.insert(
		String::from("lastStartedAt"),
		Value::Number(Number::PosInt(process.last_started_at)),
	);
	args.insert(
		String::from("createdAt"),
		Value::Number(Number::PosInt(process.created_at)),
	);

	let response = juno_module
		.call_function(&format!("{}.registerProcess", constants::APP_NAME), args)
		.await
		.unwrap();

	if let Value::Object(mut response) = response {
		if !response.contains_key("success") {
			return Err(String::from(
				"Could not find success key in the response. Malformed object",
			));
		}
		if response.remove("success").unwrap() == Value::Bool(true) {
			let module_id = response.remove("moduleId");
			if module_id.is_none() {
				return Err(String::from(
					"Could not find moduleId key in the response. Malformed object",
				));
			}
			let module_id = module_id.unwrap();
			if let Value::Number(module_id) = module_id {
				Ok(match module_id {
					Number::PosInt(module_id) => module_id,
					Number::NegInt(module_id) => module_id as u64,
					Number::Float(module_id) => module_id as u64,
				})
			} else {
				Err(format!(
					"Expected a string response for moduleId. Got: {:#?}",
					module_id
				))
			}
		} else if let Some(error) = response.remove("error") {
			Err(if let Value::String(error) = error {
				error
			} else {
				return Err(format!(
					"Expected a string response for error. Got: {:#?}",
					error
				));
			})
		} else {
			Err(String::from(
				"Expected a boolean success key in the response. Malformed object",
			))
		}
	} else {
		Err(format!(
			"Expected an object response while registering process. Got: {:#?}",
			response
		))
	}
}

fn respawn_process(mut args: HashMap<String, Value>) -> Value {
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
			.await
			.unwrap();

		let result = receiver.await.unwrap();
		if let Ok(()) = result {
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

fn add_process(mut args: HashMap<String, Value>) -> Value {
	task::block_on(async {
		let path = if let Some(Value::String(path)) = args.remove("path") {
			path
		} else {
			return generate_error_response("Path parameter is not a String");
		};

		let (sender, receiver) = channel::<Result<(), String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::AddProcess {
				node_name: String::new(),
				path,
				response: sender,
			})
			.await
			.unwrap();

		let result = receiver.await.unwrap();
		if let Ok(()) = result {
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

fn start_process(mut args: HashMap<String, Value>) -> Value {
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
			.send(GuillotineMessage::StartProcess {
				module_id,
				response: sender,
			})
			.await
			.unwrap();

		let result = receiver.await.unwrap();
		if let Ok(()) = result {
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

fn stop_process(mut args: HashMap<String, Value>) -> Value {
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
			.send(GuillotineMessage::StopProcess {
				module_id,
				response: sender,
			})
			.await
			.unwrap();

		let result = receiver.await.unwrap();
		if let Ok(()) = result {
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

fn delete_process(mut args: HashMap<String, Value>) -> Value {
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
			.send(GuillotineMessage::DeleteProcess {
				module_id,
				response: sender,
			})
			.await
			.unwrap();

		let result = receiver.await.unwrap();
		if let Ok(()) = result {
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

fn get_logs(mut args: HashMap<String, Value>) -> Value {
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

		let (sender, receiver) = channel::<Result<(String, String), String>>();
		MESSAGE_SENDER
			.read()
			.await
			.as_ref()
			.unwrap()
			.clone()
			.send(GuillotineMessage::GetProcessLogs {
				module_id,
				response: sender,
			})
			.await
			.unwrap();

		let result = receiver.await.unwrap();
		if let Ok((stdout, stderr)) = result {
			Value::Object({
				let mut map = HashMap::new();
				map.insert(String::from("success"), Value::Bool(true));
				map.insert(String::from("stdout"), Value::String(stdout));
				map.insert(String::from("stderr"), Value::String(stderr));
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
		map.insert(
			String::from("error"),
			Value::String(String::from(error_message)),
		);

		map
	})
}
