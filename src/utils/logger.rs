use colored::Colorize;

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum LogLevel {
	Verbose = 1,
	Info = 2,
	Debug = 3,
	Warn = 4,
	Error = 5,
}

impl LogLevel {
	pub fn to_string(&self) -> &str {
		match &self {
			LogLevel::Verbose => "VERBOSE",
			LogLevel::Info => "INFO",
			LogLevel::Debug => "DEBUG",
			LogLevel::Warn => "WARN",
			LogLevel::Error => "ERROR",
		}
	}
}

#[allow(dead_code)]
pub fn verbose(data: &str) {
	write(LogLevel::Verbose, data);
}

#[allow(dead_code)]
pub fn info(data: &str) {
	write(LogLevel::Info, data);
}

#[allow(dead_code)]
pub fn debug(data: &str) {
	write(LogLevel::Debug, data);
}

#[allow(dead_code)]
pub fn warn(data: &str) {
	write(LogLevel::Warn, data);
}

pub fn error(data: &str) {
	write(LogLevel::Error, data);
}

fn write(log_level: LogLevel, data: &str) {
	let log_level = match log_level {
		LogLevel::Verbose => log_level.to_string().green(),
		LogLevel::Info => log_level.to_string().blue(),
		LogLevel::Debug => log_level.to_string().yellow(),
		LogLevel::Warn => log_level.to_string().on_yellow().black(),
		LogLevel::Error => log_level.to_string().on_red().white(),
	};
	println!("[{}]: {}", log_level, data);
}
