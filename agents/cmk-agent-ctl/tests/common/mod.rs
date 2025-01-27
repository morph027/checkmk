// Copyright (C) 2019 tribe29 GmbH - License: GNU General Public License v2
// This file is part of Checkmk (https://checkmk.com). It is subject to the terms and
// conditions defined in the file COPYING, which is part of this source code package.

use cmk_agent_ctl::{certs as lib_certs, configuration::config, site_spec, types};
use std::{path::Path, str::FromStr};
pub mod agent;
pub mod certs;
use assert_cmd::Command;
#[cfg(windows)]
pub use is_elevated;

use anyhow::{anyhow, Result as AnyhowResult};

use self::certs::X509Certs;

pub fn testing_registry(
    path: &Path,
    certs: &certs::X509Certs,
    controller_uuid: uuid::Uuid,
) -> config::Registry {
    let mut registry = config::Registry::from_file(path).unwrap();
    registry.register_connection(
        &config::ConnectionType::Pull,
        &site_spec::SiteID::from_str("some_server/some_site").unwrap(),
        config::TrustedConnectionWithRemote {
            trust: config::TrustedConnection {
                uuid: controller_uuid,
                private_key: String::from_utf8(certs.controller_private_key.clone()).unwrap(),
                certificate: String::from_utf8(certs.controller_cert.clone()).unwrap(),
                root_cert: String::from_utf8(certs.ca_cert.clone()).unwrap(),
            },
            receiver_port: 1234,
        },
    );
    registry
}

pub fn testing_pull_setup(
    path: &Path,
    port: u16,
    agent_channel: types::AgentChannel,
) -> (String, config::PullConfig, certs::X509Certs) {
    let controller_uuid = uuid::Uuid::new_v4();
    let x509_certs =
        certs::X509Certs::new("Test CA", "Test receiver", &controller_uuid.to_string());
    let registry = testing_registry(
        &path.join("registered_connections.json"),
        &x509_certs,
        controller_uuid,
    );

    (
        controller_uuid.to_string(),
        testing_pull_config(port, agent_channel, registry),
        x509_certs,
    )
}

pub fn testing_pull_config(
    port: u16,
    agent_channel: types::AgentChannel,
    registry: config::Registry,
) -> config::PullConfig {
    config::PullConfig {
        allowed_ip: vec![],
        port,
        max_connections: 3,
        connection_timeout: 1,
        agent_channel,
        registry,
    }
}

pub fn testing_tls_client_connection(certs: X509Certs, address: &str) -> rustls::ClientConnection {
    let root_cert =
        lib_certs::rustls_certificate(&String::from_utf8(certs.ca_cert).unwrap()).unwrap();
    let client_cert =
        lib_certs::rustls_certificate(&String::from_utf8(certs.receiver_cert).unwrap()).unwrap();
    let private_key =
        lib_certs::rustls_private_key(&String::from_utf8(certs.receiver_private_key).unwrap())
            .unwrap();

    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.add(&root_cert).unwrap();

    let client_chain = vec![client_cert, root_cert];

    let client_config = std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store)
            .with_single_cert(client_chain, private_key)
            .unwrap(),
    );
    let server_name = rustls::client::ServerName::try_from(address).unwrap();

    rustls::ClientConnection::new(client_config, server_name).unwrap()
}

pub async fn flatten(handle: tokio::task::JoinHandle<AnyhowResult<()>>) -> AnyhowResult<()> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(anyhow!("handling failed")),
    }
}

pub fn setup_test_dir(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

#[cfg(unix)]
pub fn setup_agent_socket_path(home_dir: &std::path::Path) -> String {
    std::fs::create_dir(home_dir.join("run")).unwrap();
    home_dir
        .join("run/check-mk-agent.socket")
        .to_str()
        .unwrap()
        .to_string()
}

pub fn controller_command() -> Command {
    Command::cargo_bin("cmk-agent-ctl").unwrap()
}
