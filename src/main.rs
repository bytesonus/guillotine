extern crate async_std;
extern crate clap;
#[cfg(feature = "serde_derive")]
#[macro_use]
extern crate serde_derive;
extern crate futures;
extern crate futures_timer;
extern crate juno;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate lazy_static;

mod juno_module;
mod misc;
mod parser;
mod process_runner;
mod runner;

use async_std::{fs, path::Path};
use clap::{App, Arg, SubCommand};
use juno::JunoModule;
use std::collections::HashMap;

#[async_std::main]
async fn main() {
	let args = App::new(misc::APP_NAME)
		.version(misc::APP_VERSION)
		.author(misc::APP_AUTHORS)
		.about("Swift, painless execution")
		.subcommand(
			SubCommand::with_name("run")
				.about("Run the application with a given config file")
				.arg(
					Arg::with_name("config")
						.short("c")
						.long("config")
						.takes_value(true)
						.value_name("FILE")
						.default_value("./config.json")
						.multiple(false)
						.help("Sets the location of the config file"),
				),
		)
		.subcommand(
			SubCommand::with_name("list-processes")
				.alias("lp")
				.about("List the running processes and their statuses")
				.arg(
					Arg::with_name("config")
						.short("c")
						.long("config")
						.takes_value(true)
						.value_name("FILE")
						.default_value("./config.json")
						.multiple(false)
						.help("Sets the location of the config file"),
				),
		)
		.get_matches();

	println!("{:#?}", args);

	match args.subcommand() {
		("run", Some(args)) => {
			let config_path = Path::new(args.value_of("config").unwrap_or("./config.json"));

			if !config_path.exists().await {
				println!(
					"Config file {} doesn't exist. Quitting.",
					config_path.to_string_lossy()
				);
				return;
			}
			let file_contents = fs::read_to_string(config_path).await;
			if let Err(err) = file_contents {
				println!("Error reading config file: {}", err);
				return;
			}
			let config_result = parser::select_config(file_contents.unwrap()).await;
			if let Err(err) = config_result {
				println!("Error selecting a configuration to run: {}", err);
				return;
			}
			let config = config_result.unwrap();
			runner::run(config).await;
		}
		("list-processes", Some(args)) => {
			let config_path = Path::new(args.value_of("config").unwrap_or("./config.json"));

			if !config_path.exists().await {
				println!(
					"Config file {} doesn't exist. Quitting.",
					config_path.to_string_lossy()
				);
				return;
			}
			let file_contents = fs::read_to_string(config_path).await;
			if let Err(err) = file_contents {
				println!("Error reading config file: {}", err);
				return;
			}
			let config_result = parser::select_config(file_contents.unwrap()).await;
			if let Err(err) = config_result {
				println!("Error selecting a configuration to run: {}", err);
				return;
			}
			let config = config_result.unwrap();

			let mut module = if config.juno.connection_type == "unix_socket" {
				let socket_path = config.juno.socket_path.as_ref().unwrap();
				JunoModule::from_unix_socket(&socket_path)
			} else {
				let port = config.juno.port.as_ref().unwrap();
				let bind_addr = config.juno.bind_addr.as_ref().unwrap();
				JunoModule::from_inet_socket(&bind_addr, *port)
			};

			module
				.initialize(
					&format!("{}-cli", misc::APP_NAME),
					misc::APP_VERSION,
					HashMap::new(),
				)
				.await
				.unwrap();
			let processes = module
				.call_function(&format!("{}.listProcesses", misc::APP_NAME), HashMap::new())
				.await
				.unwrap();
			println!("{:#?}", processes);
		}
		(cmd, _) => println!("Unknown command '{}'", cmd),
	}
}
