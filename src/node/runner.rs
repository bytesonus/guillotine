use crate::{
	models::{GuillotineMessage, NodeConfig, RunnerConfig},
	node::{juno_module, module::get_module_from_path, process::Process},
	utils::{constants, logger},
};

use async_std::{fs, net::TcpStream, path::Path, sync::Mutex};
use futures::{channel::mpsc::unbounded, StreamExt};
use futures_timer::Delay;
use std::{collections::HashMap, io::Error, time::Duration};

lazy_static! {
	static ref CLOSE_FLAG: Mutex<bool> = Mutex::new(false);
}

pub async fn run(config: RunnerConfig) {
	if config.name.is_none() {
		logger::error("Node name cannot be null");
		return;
	}

	let node_name = config.name.unwrap();
	let node = config.node.take().unwrap();
	let log_dir = config.logs;

	if !try_connecting_to_host(&node).await {
		logger::error(&format!(
			"Could not connect to the host instance of {}. Please check your settings",
			constants::APP_NAME
		));
		return;
	}

	// Populate any auto-start modules here
	let mut auto_start_processes = vec![];
	if config.modules.is_some() {
		let modules_dir = config.modules.unwrap().directory;
		let modules_path = Path::new(&modules_dir);

		if modules_path.exists().await && modules_path.is_dir().await {
			let mut dir_iterator = fs::read_dir(modules_dir).await.unwrap();
			while let Some(Ok(dir)) = dir_iterator.next().await {
				if let Some(process) =
					get_module_from_path(dir.path().to_str().unwrap(), log_dir.clone()).await
				{
					auto_start_processes.push(process);
				}
			}
		}
	}

	keep_node_alive(node_name, node, auto_start_processes).await;
}

pub async fn on_exit() {
	*CLOSE_FLAG.lock().await = true;
}

async fn keep_node_alive(node_name: String, node: NodeConfig, auto_start_processes: Vec<Process>) {
	// Initialize the guillotine juno module
	let (sender, mut command_receiver) = unbounded::<GuillotineMessage>();
	let response = juno_module::setup_module(&node_name, &node, sender.clone()).await;
	if let Err(error) = response {
		logger::error(&format!("Error setting up Juno module: {}", error));
		return;
	}
	let juno_module = response.unwrap();

	let ids_to_processes = HashMap::new();

	// First, register all the auto_start_processes
	for process in auto_start_processes {
		let response = juno_module::register_module(&node_name, &juno_module, &process).await;
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

	// Then iterate through the commands recieved by the juno module and do something about it
	while let Some(command) = command_receiver.next().await {
		
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
		return true;
	} else if node.connection_type == constants::connection_type::UNIX_SOCKET {
		let unix_socket = node.socket_path.unwrap();
		let mut connection = connect_to_unix_socket(&unix_socket).await;
		if connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(1000)).await;
			connection = connect_to_unix_socket(&unix_socket).await;
			return connection.is_ok();
		}
		return true;
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
