use crate::{
	exec::{juno_module, process::ProcessRunner},
	models::{GuillotineMessage, HostConfig},
	utils::logger,
};
use std::{
	collections::HashMap,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_std::{io::Error, net::TcpStream, sync::Mutex, task};
use futures::{
	channel::mpsc::unbounded,
	future::{self, Either},
	StreamExt,
};
use futures_timer::Delay;
use juno::models::{Number, Value};

lazy_static! {
	static ref CLOSE_FLAG: Mutex<bool> = Mutex::new(false);
}

pub async fn run(mut juno_process: ProcessRunner, juno_config: HostConfig) {
	// Spawn juno before spawing any modules
	while !juno_process.is_process_running() {
		juno_process.respawn().await;
		ensure_juno_initialized(&juno_config).await;
	}

	// Initialize the guillotine juno module
	let (mut sender, mut command_receiver) = unbounded::<GuillotineMessage>();
	let mut juno_module = juno_module::setup_host_module(&juno_config, sender).await;

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
					juno_module.close().await;
					drop(juno_module);
					juno_process.respawn().await;
					ensure_juno_initialized(&juno_config).await;

					let channel = unbounded::<GuillotineMessage>();
					sender = channel.0;
					command_receiver = channel.1;

					command_future = command_receiver.next();
					juno_module = juno_module::setup_host_module(&juno_config, sender).await;
				}

				for module in processes.iter_mut() {
					// If a module isn't running, respawn it. Simple.
					if module.get_runner() == "host" {
						if !module.is_process_running() {
							module.respawn().await;
						}
					} else {
						// The module belongs to another node.
						let result = juno_module
							.call_function(
								&format!(
									"guillotine-node-{}.isProcessRunning",
									module.get_runner()
								),
								HashMap::new(),
							)
							.await;
						if result.is_err() {
							continue;
						}
						if result.unwrap() == Value::Bool(false) {
							let _ = juno_module
								.call_function(
									&format!(
										"guillotine-node-{}.respawnProcess",
										module.get_runner()
									),
									vec![(
										String::from("pid"),
										Value::Number(Number::PosInt(module.get_module_id())),
									)]
									.into_iter()
									.collect(),
								)
								.await;
						}
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
							processes
								.iter()
								.for_each(|process| runners.push(process.copy()));
							sender.send(runners).unwrap();
						}
						GuillotineMessage::RestartProcess(pid, response_sender) => {
							if pid == 0 {
								response_sender.send(true).unwrap();
								juno_module.close().await;
								drop(juno_module);
								juno_process.respawn().await;
								ensure_juno_initialized(&juno_config).await;

								let channel = unbounded::<GuillotineMessage>();
								sender = channel.0;
								command_receiver = channel.1;

								command_future = command_receiver.next();
								juno_module =
									juno_module::setup_host_module(&juno_config, sender).await;
								continue;
							}

							let module = processes
								.iter_mut()
								.find(|process| process.get_module_id() == pid);
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
	processes.iter_mut().for_each(|module| {
		logger::info(&format!("Quitting process: {}", module.get_name()));
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
				logger::info(&format!("Killing process: {}", module.get_name()));
				module.kill();
			});
			break;
		}
	}

	// Now quit juno similarly
	logger::info(&format!("Quitting process: {}", juno_process.get_name()));
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
			logger::info(&format!("Killing process: {}", juno_process.get_name()));
			juno_process.kill();
			break;
		}
	}
}

pub async fn on_exit() {
	*CLOSE_FLAG.lock().await = true;
}

async fn ensure_juno_initialized(host: &HostConfig) {
	if host.port.is_some() {
		let port = host.port.unwrap();
		// Attempt to connect to the port until you can connect
		let port = format!("127.0.0.1:{}", port);
		let mut connection = TcpStream::connect(&port).await;
		if connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(1000)).await;
			connection = TcpStream::connect(&port).await;
		}
	} else {
		let unix_socket = host.socket_path.unwrap();
		let mut connection = connect_to_unix_socket(&unix_socket).await;
		if connection.is_err() {
			// If connection failed, wait and try again
			Delay::new(Duration::from_millis(1000)).await;
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
