# Security Policy

## Supported Versions

TermHub is pre-1.0 software. Security fixes are provided for the latest commit on the default branch and the latest GitHub release, when releases are available.

## Reporting a Vulnerability

Please do not open a public issue for vulnerabilities.

Report security issues by emailing the maintainer or by using GitHub private vulnerability reporting if it is enabled for this repository. Include:

- Affected version or commit
- Operating system and installation method
- Reproduction steps
- Expected and actual impact
- Logs or screenshots with secrets removed

## Security Notes

TermHub is designed for trusted LAN collaboration. By default, SSH session listeners may bind to `0.0.0.0`, which allows other machines on the network to connect if they know the session username, password, and port.

Before exposing TermHub outside a trusted LAN:

- Change listeners to `127.0.0.1` or firewall the port.
- Use strong per-session passwords.
- Avoid reusing SSH upstream passwords.
- Do not publish `%APPDATA%/termhub/config.toml`, host keys, logs, or screenshots containing session details.

The local HTTP API used by `termhub-cli` defaults to `127.0.0.1:2223` and should remain bound to loopback unless you understand the risk.
