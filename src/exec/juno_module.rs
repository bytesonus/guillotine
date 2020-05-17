use crate::{
	exec::process::ProcessRunner,
	models::{GuillotineMessage, HostConfig, ModuleRunningStatus},
	utils::constants,
};
use std::{collections::HashMap, sync::Mutex};

use async_std::task;
use futures::{
	channel::{mpsc::UnboundedSender, oneshot::channel},
	SinkExt,
};
use juno::{
	models::{Number, Value},
	JunoModule,
};

lazy_static! {
	static ref MESSAGE_SENDER: Mutex<Option<UnboundedSender<GuillotineMessage>>> = Mutex::new(None);
}

pub async fn setup_host_module(
	config: &HostConfig,
	sender: UnboundedSender<GuillotineMessage>,
) -> JunoModule {
	MESSAGE_SENDER.lock().unwrap().replace(sender);

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
		.declare_function("listProcesses", list_processes)
		.await
		.unwrap();

	module
		.declare_function("restartProcess", restart_process)
		.await
		.unwrap();

	module
}

fn list_processes(_: HashMap<String, Value>) -> Value {
	let message_sender = MESSAGE_SENDER.lock().unwrap();
	let mut message_sender = message_sender.as_ref().unwrap();

	let (sender, receiver) = channel::<Vec<ProcessRunner>>();

	task::block_on(message_sender.send(GuillotineMessage::ListProcesses(sender))).unwrap();

	Value::Array(
		task::block_on(receiver)
			.unwrap()
			.into_iter()
			.map(|process| {
				let mut map = HashMap::new();

				map.insert(
					String::from("id"),
					Value::Number(Number::PosInt(process.get_module_id())),
				);
				map.insert(
					String::from("name"),
					Value::String(process.get_name().to_string()),
				);
				map.insert(
					String::from("status"),
					Value::String(String::from(match process.status {
						ModuleRunningStatus::Running => "running",
						ModuleRunningStatus::Offline => "offline",
					})),
				);
				map.insert(
					String::from("restarts"),
					Value::Number(Number::NegInt(process.restarts)),
				);
				map.insert(
					String::from("uptime"),
					Value::Number(Number::PosInt(process.uptime)),
				);
				map.insert(
					String::from("crashes"),
					Value::Number(Number::PosInt(process.crashes)),
				);
				map.insert(
					String::from("createdAt"),
					Value::Number(Number::PosInt(process.created_at)),
				);

				Value::Object(map)
			})
			.collect(),
	)
}

fn restart_process(args: HashMap<String, Value>) -> Value {
	let pid = args.get("processId");
	if pid.is_none() {
		return Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(false));
			map.insert(
				String::from("error"),
				Value::String(String::from("No PID supplied")),
			);
			map
		});
	}
	let pid = pid.unwrap().as_number();
	if pid.is_none() {
		return Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(false));
			map.insert(
				String::from("error"),
				Value::String(String::from("PID supplied is not a number")),
			);
			map
		});
	}
	let pid = match pid.unwrap() {
		Number::Float(num) => *num as u64,
		Number::NegInt(num) => *num as u64,
		Number::PosInt(num) => *num,
	};

	let message_sender = MESSAGE_SENDER.lock().unwrap();
	let mut message_sender = message_sender.as_ref().unwrap();

	let (sender, receiver) = channel::<bool>();

	task::block_on(message_sender.send(GuillotineMessage::RestartProcess(pid, sender))).unwrap();
	let found_process = task::block_on(receiver).unwrap();

	if found_process {
		Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(true));
			map
		})
	} else {
		Value::Object({
			let mut map = HashMap::new();
			map.insert(String::from("success"), Value::Bool(false));
			map.insert(
				String::from("error"),
				Value::String(String::from("No process found with that PID")),
			);
			map
		})
	}
}
