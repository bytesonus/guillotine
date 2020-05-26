#[cfg(feature = "serde_derive")]
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
extern crate async_std;
extern crate chrono;
extern crate clap;
extern crate cli_table;
extern crate colored;
extern crate ctrlc;
extern crate futures;
extern crate futures_timer;
extern crate juno;
extern crate serde;
extern crate serde_json;

#[cfg(target_family = "unix")]
extern crate nix;

#[cfg(target_family = "windows")]
extern crate winapi;

mod cli;
mod host;
mod models;
mod node;
mod runner;
mod utils;

use models::parser;
use utils::{constants, logger};

use async_std::{fs, path::Path, task};
use clap::{App, Arg, SubCommand};
use futures::future;

#[async_std::main]
async fn main() {
	let args = App::new(constants::APP_NAME)
		.version(constants::APP_VERSION)
		.author(constants::APP_AUTHORS)
		.about(constants::APP_ABOUT)
		.subcommand(
			SubCommand::with_name("run").about("Run the application with a given config file"),
		)
		.subcommand(
			SubCommand::with_name("list-modules")
				.alias("lm")
				.about("List the modules connected and their statuses"),
		)
		.subcommand(
			SubCommand::with_name("list-nodes")
				.alias("ln")
				.about("List all the nodes registered with this host and their details"),
		)
		.subcommand(
			SubCommand::with_name("list-all-processes")
				.alias("lap")
				.about("List all running processes and their states across all nodes"),
		)
		.subcommand(
			SubCommand::with_name("list-processes")
				.alias("lp")
				.about("List the running processes and their statuses for a given node")
				.arg(
					Arg::with_name("node")
						.short("n")
						.long("node")
						.takes_value(true)
						.required(false),
				),
		)
		.subcommand(
			SubCommand::with_name("restart")
				.about("Restarts a process with a processId")
				.arg(
					Arg::with_name("pid")
						.takes_value(true)
						.required(true)
						.allow_hyphen_values(false),
				),
		)
		.subcommand(
			SubCommand::with_name("info")
				.alias("i")
				.about("Get information about a process / module")
				.arg(
					Arg::with_name("pid")
						.takes_value(true)
						.required(true)
						.allow_hyphen_values(false),
				),
		)
		.arg(
			Arg::with_name("config")
				.short("c")
				.long("config")
				.takes_value(true)
				.value_name("FILE")
				.default_value("./config.json")
				.multiple(false)
				.help("Sets the location of the config file"),
		)
		.get_matches();

	ctrlc::set_handler(|| task::block_on(on_exit())).expect("Error setting the CtrlC handler");

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

	match args.subcommand() {
		// Host or node stuff
		("run", Some(_)) | ("", _) => runner::run(config).await,

		// Cli stuff
		("list-modules", Some(_)) => cli::list_modules(config).await,
		("list-nodes", Some(_)) => cli::list_nodes(config).await,
		("list-all-processes", Some(_)) => cli::list_all_processes(config).await,
		("list-processes", Some(args)) => cli::list_processes(config, args).await,
		("info", Some(args)) => cli::get_info(config, args).await,
		("restart", Some(args)) => cli::restart_process(config, args).await,

		(cmd, _) => println!("Unknown command '{}'", cmd),
	}
}

async fn on_exit() {
	logger::info("Received exit code. Closing all modules");
	future::join(runner::on_exit(), cli::on_exit()).await;
}
