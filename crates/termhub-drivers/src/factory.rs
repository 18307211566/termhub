use termhub_core::{UpstreamDriver, UpstreamSpec};

use crate::eol::EolMode;
use crate::local_shell::LocalShellDriver;
use crate::raw_tcp::RawTcpDriver;
use crate::serial::{parse_serial_params, SerialDriver};
use crate::ssh_client::SshClientDriver;
use crate::telnet::TelnetDriver;

pub fn create_driver(spec: &UpstreamSpec) -> anyhow::Result<Box<dyn UpstreamDriver>> {
    Ok(match spec {
        UpstreamSpec::Loopback => Box::new(crate::loopback::LoopbackDriver::default()),
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
            input_eol: EolMode::from_str(input_eol),
            output_eol: EolMode::from_str(output_eol),
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
    })
}
