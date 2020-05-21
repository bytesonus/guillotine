use crate::{cli::get_juno_module_from_config, logger, models::RunnerConfig, utils::constants};

use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use std::collections::HashMap;

pub async fn list_processes(config: RunnerConfig) {
	let result = get_juno_module_from_config(&config);
	let mut module = if let Ok(module) = result {
		module
	} else {
		logger::error(if let Err(err) = result {
			err
		} else {
			return;
		});
		return;
	};

	module
		.initialize(
			&format!("{}-cli", constants::APP_NAME),
			constants::APP_VERSION,
			HashMap::new(),
		)
		.await
		.unwrap();
	let processes = module
		.call_function(
			&format!("{}.listAllProcesses", constants::APP_NAME),
			HashMap::new(),
		)
		.await
		.unwrap();
	if !processes.is_array() {
		logger::error(&format!("Expected array response. Got {:?}", processes));
		return;
	}
	let processes = processes.as_array().unwrap();

	// Make the looks first
	let header_format = CellFormat::builder()
		.align(Align::Center)
		.bold(true)
		.underline(true)
		.build();
	let table_format = TableFormat::new(
		Border::builder()
			.top(HorizontalLine::new('┌', '┐', '┬', '─'))
			.bottom(HorizontalLine::new('└', '┘', '┴', '─'))
			.right(VerticalLine::new('│'))
			.left(VerticalLine::new('│'))
			.build(),
		Separator::builder()
			.row(None) //Use this for a line: Some(HorizontalLine::new('├', '┤', '┼', '─')))
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
				&super::get_duration(
					process
						.get("uptime")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap(),
				),
				Default::default(),
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
				&super::get_date_time(
					process
						.get("createdAt")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap(),
				),
				Default::default(),
			),
		]));
	}
	let table = Table::new(table_data, table_format);

	// Print it out
	table.unwrap().print_stdout().unwrap();
}
