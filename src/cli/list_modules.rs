use crate::{logger, models::GuillotineSpecificConfig, utils::constants};

use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use juno::JunoModule;
use std::collections::HashMap;

pub async fn list_modules(config: GuillotineSpecificConfig) {
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
			&format!("{}-cli", constants::APP_NAME),
			constants::APP_VERSION,
			HashMap::new(),
		)
		.await
		.unwrap();
	let modules = module
		.call_function("juno.listModules", HashMap::new())
		.await
		.unwrap();
	if !modules.is_array() {
		logger::error(&format!("Expected array response. Got {:?}", modules));
		return;
	}
	let modules = modules.as_array().unwrap();

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
		Cell::new("Module ID", header_format),
		Cell::new("Version", header_format),
		Cell::new("Status", header_format),
	])];
	for process in modules.iter() {
		let process = process.as_object().unwrap();
		table_data.push(Row::new(vec![
			Cell::new(
				process.get("moduleId").unwrap().as_string().unwrap(),
				Default::default(),
			),
			Cell::new(
				process.get("version").unwrap().as_string().unwrap(),
				Default::default(),
			),
			match process.get("registered").unwrap().as_bool().unwrap() {
				true => Cell::new(
					"active",
					CellFormat::builder()
						.foreground_color(Some(Color::Green))
						.build(),
				),
				false => Cell::new(
					"inactive",
					CellFormat::builder()
						.foreground_color(Some(Color::Cyan))
						.build(),
				),
			},
		]));
	}
	let table = Table::new(table_data, table_format);

	// Print it out
	table.unwrap().print_stdout().unwrap();
}
