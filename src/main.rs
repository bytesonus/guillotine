extern crate async_std;
extern crate clap;
extern crate serde_json;

mod constants;
mod parser;

use async_std::{fs, path::Path, task};
use clap::{App, Arg};
use serde_json::from_str;
use std::{process::Command, time::Duration};

#[async_std::main]
async fn main() {
	let args = App::new(constants::APP_NAME)
		.version(constants::APP_VERSION)
		.author(constants::APP_AUTHORS)
		.about("Swift, painless execution")
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

	let file_contents = file_contents.unwrap();
	let parsed_config = from_str(&file_contents);
	if let Err(err) = parsed_config {
		println!("Config file could not be parsed: {}", err);
		return;
	}

	let config_result = parser::select_config(parsed_config.unwrap()).await;
	if let Err(err) = config_result {
		println!("Error selecting a configuration to run: {}", err);
		return;
	}
	let config = config_result.unwrap();
	let gotham_path = config.get("gotham").unwrap().as_str().unwrap().to_string();

	let mut gotham_process = if config.get("connection_type").unwrap() == "unix_socket" {
		let socket_path = config.get("path").unwrap().as_str().unwrap().to_string();
		Command::new(gotham_path)
			.arg("--socket-location")
			.arg(socket_path)
			.spawn()
			.expect("Failed to execute gotham")
	} else {
		let port = config.get("port").unwrap().as_u64().unwrap();
		let bind_addr = config
			.get("bind-addr")
			.unwrap()
			.as_str()
			.unwrap()
			.to_string();
		Command::new(gotham_path)
			.arg("--port")
			.arg(format!("{}", port))
			.arg("--bind-addr")
			.arg(bind_addr)
			.spawn()
			.expect("Failed to execute gotham")
	};

	loop {
		match gotham_process.try_wait() {
			Ok(Some(status)) => println!("Exited with: {}", status),
			Ok(None) => {
				let res = gotham_process.wait();
				println!("Exited with: {:?}", res);
			}
			Err(e) => println!("Error attempting to wait: {}", e),
		}
		task::sleep(Duration::from_millis(500)).await;
	}
}
