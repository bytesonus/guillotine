use crate::{
	misc,
	misc::GuillotineMessage,
	parser::ConfigValue,
	process_runner::{ModuleRunningStatus, ProcessRunner},
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

pub async fn setup_module(config: ConfigValue, sender: UnboundedSender<GuillotineMessage>) {
	let mut message_sender = MESSAGE_SENDER.lock().unwrap();
	*message_sender = Some(sender);
	drop(message_sender);

	let mut module = if config.juno.connection_type == "unix_socket" {
		let socket_path = config.juno.socket_path.as_ref().unwrap();
		JunoModule::from_unix_socket(&socket_path)
	} else {
		let port = config.juno.port.as_ref().unwrap();
		let bind_addr = config.juno.bind_addr.as_ref().unwrap();

		JunoModule::from_inet_socket(&bind_addr, *port)
	};

	module
		.initialize(misc::APP_NAME, misc::APP_VERSION, HashMap::new())
		.await
		.expect("Could not initialize Guillotine Juno Module");

	module
		.declare_function("listProcesses", list_processes)
		.await
		.unwrap();
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
					Value::Number(Number::PosInt(process.module_id)),
				);
				map.insert(String::from("name"), Value::String(process.config.name));
				map.insert(
					String::from("status"),
					Value::String(String::from(match process.status {
						ModuleRunningStatus::Running => "running",
						ModuleRunningStatus::Offline => "offline",
					})),
				);
				map.insert(
					String::from("restarts"),
					Value::Number(Number::PosInt(process.restarts)),
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
