use crate::{cli::get_juno_module_from_config, logger, models::RunnerConfig, utils::constants};

use cli_table::{
	format::{
		Align,
		Border,
		CellFormat,
		Color,
		HorizontalLine,
		Separator,
		TableFormat,
		VerticalLine,
	},
	Cell,
	Row,
	Table,
};
use std::collections::HashMap;

pub async fn list_modules(config: RunnerConfig) {
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
