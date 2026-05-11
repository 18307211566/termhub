use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

pub fn default_config_dir() -> anyhow::Result<PathBuf> {
    if let Some(p) = dirs::config_dir() {
        return Ok(p.join("termhub"));
    }
    anyhow::bail!("cannot resolve config dir")
}

#[doc(hidden)]
pub fn config_dir_for_test() -> PathBuf {
    std::env::temp_dir().join("termhub-test-unused")
}

fn redact_for_save(cfg: &Config) -> anyhow::Result<Config> {
    let mut c = cfg.clone();
    for s in c.sessions.iter_mut() {
        s.password = crate::secrets::encrypt(&s.password)?;
        if let crate::UpstreamSpec::Ssh { password, .. } = &mut s.upstream {
            *password = crate::secrets::encrypt(password)?;
        }
    }
    Ok(c)
}

fn unredact_after_load(cfg: &mut Config) -> anyhow::Result<()> {
    for s in cfg.sessions.iter_mut() {
        s.password = crate::secrets::decrypt(&s.password)?;
        if let crate::UpstreamSpec::Ssh { password, .. } = &mut s.upstream {
            *password = crate::secrets::decrypt(password)?;
        }
    }
    Ok(())
}

pub fn load_or_default(dir: &Path) -> anyhow::Result<Config> {
    let path = dir.join("config.toml");
    if !path.exists() {
        return Ok(Config {
            server: crate::ServerConfig {
                listen: "0.0.0.0:2222".into(),
                host_key_path: "host_key".into(),
                max_clients_per_session: 16,
                api_listen: "127.0.0.1:2223".into(),
            },
            ui: crate::config::UiConfig::default(),
            sessions: vec![],
        });
    }
    let s = fs::read_to_string(&path)?;
    let mut cfg: Config = toml::from_str(&s)?;
    unredact_after_load(&mut cfg)?;
    Ok(cfg)
}

pub fn save(dir: &Path, cfg: &Config) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    let to_write = redact_for_save(cfg)?;
    let s = toml::to_string_pretty(&to_write)?;
    let path = dir.join("config.toml");
    let tmp = dir.join("config.toml.tmp");
    fs::write(&tmp, s)?;
    fs::rename(tmp, path)?;
    Ok(())
}
