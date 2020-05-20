use crate::{
	host,
	models::{NodeConfig, RunnerConfig},
	node,
};
use async_std::{fs, path::Path};
use futures::future::join;

pub async fn run(mut config: RunnerConfig) {
	if config.logs.is_some() {
		let log_dir = config.logs.as_ref().unwrap();
		let main_dir = Path::new(log_dir);
		if !main_dir.exists().await {
			fs::create_dir(&main_dir).await.unwrap();
		}
	}

	if config.host.is_some() {
		let host = config.host.as_ref().unwrap();
		if config.name.is_none() {
			config.name = Some(String::from("host"));
		}
		// If the host doesn't have a node, create one
		config.node = Some(NodeConfig {
			connection_type: host.connection_type.clone(),
			ip: Some(String::from("127.0.0.1")),
			port: host.port,
			socket_path: host.socket_path.clone(),
		});

		join(host::run(config.clone()), node::run(config)).await;
	} else if config.node.is_some() {
		node::run(config).await;
	} else {
		return;
	}
}

pub async fn on_exit() {
	futures::join!(host::on_exit(), node::on_exit());
}
