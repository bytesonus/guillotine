mod get_module_info;
mod list_modules;
mod list_processes;
mod restart_process;

pub use get_module_info::get_module_info;
pub use list_modules::list_modules;
pub use list_processes::list_processes;
pub use restart_process::restart_process;

use crate::models::RunnerConfig;
use chrono::{prelude::*, Utc};
use juno::JunoModule;

pub async fn on_exit() {}

fn get_juno_module_from_config(config: &RunnerConfig) -> Result<JunoModule, &str> {
	if let Some(host) = &config.host {
		if host.connection_type == "unix_socket" {
			let socket_path = host.socket_path.as_ref().unwrap();
			Ok(JunoModule::from_unix_socket(socket_path))
		} else {
			let port = host.port.as_ref().unwrap();
			let bind_addr = host.bind_addr.as_ref().unwrap();
			Ok(JunoModule::from_inet_socket(&bind_addr, *port))
		}
	} else if let Some(node) = &config.node {
		if node.connection_type == "unix_socket" {
			let socket_path = node.socket_path.as_ref().unwrap();
			Ok(JunoModule::from_unix_socket(socket_path))
		} else {
			let port = node.port.as_ref().unwrap();
			let ip = node.ip.as_ref().unwrap();
			Ok(JunoModule::from_inet_socket(&ip, *port))
		}
	} else {
		Err("Neither host, nor node are set. Invalid configuration found")
	}
}

fn get_date_time(timestamp: i64) -> String {
	Utc.timestamp_millis(timestamp)
		.format("%a %b %e %T %Y")
		.to_string()
}

fn get_duration(mut duration: i64) -> String {
	// If less than 1 second, print the ms
	if (duration / 1000) <= 0 {
		return format!("{}ms", duration);
	}
	duration /= 1000; // Seconds

	// If less than 60 seconds, print the seconds
	if (duration / 60) <= 0 {
		return format!("{}s", duration);
	}
	duration /= 60; // Minutes

	// If less than 60 minutes, print the minutes
	if (duration / 60) <= 0 {
		return format!("{}m", duration);
	}
	duration /= 60; // Hours

	// If less than 24 hours, print the hours
	if (duration / 24) <= 0 {
		return format!("{}h", duration);
	}
	duration /= 24; // Days

	// If less than 7 days, print the days
	if (duration / 7) <= 0 {
		return format!("{}d", duration);
	}
	// duration still represents Days

	// If less than 30 days, print the weeks
	if (duration / 30) <= 0 {
		return format!("{}w", duration / 7);
	}
	duration /= 30; // Months

	// If less than 12 months, print the months
	if (duration / 12) <= 0 {
		return format!("{}M", duration);
	}
	duration /= 12; // Years

	// Neither of those works, print the number of years
	return format!("{}y", duration);
}
