use crate::{logger, misc, parser::ConfigValue};

use chrono::{prelude::*, Utc};
use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use juno::JunoModule;
use std::collections::HashMap;

pub async fn list_processes(config: ConfigValue) {
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
	if !processes.is_array() {
		logger::error(&format!("Expected array response. Got {:?}", processes));
	}
	let processes = processes.as_array().unwrap();

	// Make the looks first
	let header_format = CellFormat::builder()
		.align(Align::Center)
		.bold(true)
		.build();
	let table_format = TableFormat::new(
		Border::builder()
			.top(HorizontalLine::new('┌', '┐', '┬', '─'))
			.bottom(HorizontalLine::new('└', '┘', '┴', '─'))
			.right(VerticalLine::new('│'))
			.left(VerticalLine::new('│'))
			.build(),
		Separator::builder()
			.row(Some(HorizontalLine::new('├', '┤', '┼', '─')))
			.column(Some(VerticalLine::new('│')))
			.build(),
	);

	// Now make the data
	let mut table_data = vec![Row::new(vec![
		Cell::new("ID", header_format),
		Cell::new("Name", header_format),
		Cell::new("Status", header_format),
		Cell::new("Restarts", header_format),
		Cell::new("Uptime", header_format),
		Cell::new("Crashes", header_format),
		Cell::new("Created at", header_format),
	])];
	for process in processes.iter() {
		let process = process.as_object().unwrap();
		table_data.push(Row::new(vec![
			Cell::new(
				&format!(
					"{}",
					process
						.get("id")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
			Cell::new(
				process.get("name").unwrap().as_string().unwrap(),
				Default::default(),
			),
			match process.get("status").unwrap().as_string().unwrap().as_ref() {
				"running" => Cell::new(
					"running",
					CellFormat::builder()
						.foreground_color(Some(Color::Green))
						.build(),
				),
				"offline" => Cell::new(
					"offline",
					CellFormat::builder()
						.foreground_color(Some(Color::Red))
						.build(),
				),
				_ => Cell::new(
					"unknown",
					CellFormat::builder()
						.foreground_color(Some(Color::Cyan))
						.build(),
				),
			},
			Cell::new(
				&format!(
					"{}",
					process
						.get("restarts")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
			Cell::new(
				&get_duration(
					process
						.get("uptime")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap(),
				),
				header_format,
			),
			Cell::new(
				&format!(
					"{}",
					process
						.get("crashes")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
			Cell::new(
				&get_date_time(
					process
						.get("createdAt")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap(),
				),
				header_format,
			),
		]));
	}
	let table = Table::new(table_data, table_format);

	// Print it out
	table.unwrap().print_stdout().unwrap();
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
