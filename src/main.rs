extern crate async_std;
extern crate clap;
#[cfg(feature = "serde_derive")]
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

mod constants;
mod parser;
mod runner;

use async_std::{fs, path::Path};
use clap::{App, Arg};

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

	let config_result = parser::select_config(file_contents.unwrap()).await;
	if let Err(err) = config_result {
		println!("Error selecting a configuration to run: {}", err);
		return;
	}
	let config = config_result.unwrap();

	runner::run(config).await;
}
