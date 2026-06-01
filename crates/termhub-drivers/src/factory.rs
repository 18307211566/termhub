use termhub_core::{UpstreamDriver, UpstreamSpec};

use crate::eol::EolMode;
use crate::local_shell::LocalShellDriver;
use crate::raw_tcp::RawTcpDriver;
use crate::serial::{parse_serial_params, SerialDriver};
use crate::ssh_client::SshClientDriver;
use crate::telnet::TelnetDriver;

pub fn create_driver(spec: &UpstreamSpec) -> anyhow::Result<Box<dyn UpstreamDriver>> {
    Ok(match spec {
        UpstreamSpec::Loopback => Box::new(crate::loopback::LoopbackDriver),
        UpstreamSpec::Serial {
            port,
            baud,
            data_bits,
            parity,
            stop_bits,
            flow,
            input_eol,
            output_eol,
        } => Box::new(SerialDriver {
            port: port.clone(),
            baud: *baud,
            params: parse_serial_params(*data_bits, parity, *stop_bits, flow)?,
            input_eol: EolMode::parse(input_eol),
            output_eol: EolMode::parse(output_eol),
        }),
        UpstreamSpec::Ssh {
            host,
            port,
            user,
            password,
        } => Box::new(SshClientDriver {
            host: host.clone(),
            port: *port,
            user: user.clone(),
            password: password.clone(),
        }),
        UpstreamSpec::Telnet { host, port } => Box::new(TelnetDriver {
            host: host.clone(),
            port: *port,
        }),
        UpstreamSpec::RawTcp { host, port } => Box::new(RawTcpDriver {
            host: host.clone(),
            port: *port,
        }),
        UpstreamSpec::LocalShell { command, args } => Box::new(LocalShellDriver {
            command: command.clone(),
            args: args.clone(),
            cols: 80,
            rows: 24,
        }),
        UpstreamSpec::HttpProxy { .. } => {
            anyhow::bail!(
                "HttpProxy does not use UpstreamDriver; it runs as a standalone TCP proxy"
            )
        }
    })
}
