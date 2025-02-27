//! Configuration related structures
use clap::{crate_version, AppSettings, Clap};
use failure::{format_err, Fallible};
use getset::Getters;
use ipnetwork::Ipv4Network;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, canonicalize, create_dir_all, read_to_string},
    path::PathBuf,
};
use toml;

#[derive(Clap, Deserialize, Getters, Serialize)]
#[serde(rename_all = "kebab-case")]
#[clap(
    after_help = "More info at: https://github.com/saschagrunert/kubernix",
    author = "Sascha Grunert <mail@saschagrunert.de>",
    raw(global_setting = "AppSettings::ColoredHelp"),
    raw(version = "crate_version!()")
)]
/// The global configuration
pub struct Config {
    #[get = "pub"]
    #[clap(subcommand)]
    /// All available subcommands
    subcommand: Option<SubCommand>,

    #[get = "pub"]
    #[clap(
        default_value = "kubernix-run",
        env = "KUBERNIX_RUN",
        global = true,
        help = "Path where all the runtime data is stored",
        long = "root",
        short = "r",
        value_name = "PATH"
    )]
    /// The root path during runtime
    root: PathBuf,

    #[get = "pub"]
    #[clap(
        default_value = "info",
        env = "KUBERNIX_LOG_LEVEL",
        help = "Set the log level verbosity",
        long = "log-level",
        raw(possible_values = r#"&["trace", "debug", "info", "warn", "error", "off"]"#),
        short = "l",
        value_name = "LEVEL"
    )]
    /// The logging level of the application
    log_level: LevelFilter,

    #[get = "pub"]
    #[clap(
        default_value = "10.10.0.0/16",
        env = "KUBERNIX_CIDR",
        help = "The CIDR used for the cluster",
        long = "cidr",
        short = "c",
        value_name = "CIDR"
    )]
    /// The CIDR used for the cluster
    cidr: Ipv4Network,

    #[get = "pub"]
    #[clap(
        env = "KUBERNIX_OVERLAY",
        help = "The Nix package overlay to be used",
        long = "overlay",
        short = "o",
        value_name = "PATH"
    )]
    /// The Nix package overlay to be used
    overlay: Option<PathBuf>,

    #[get = "pub"]
    #[clap(
        help = "Do not clear the current env during bootstrap",
        long = "impure",
        short = "i"
    )]
    /// Do not clear the current env during bootstrap
    impure: bool,

    #[get = "pub"]
    #[clap(
        env = "KUBERNIX_PACKAGES",
        help = "Additional Nix dependencies to be added to the environment",
        long = "packages",
        multiple = true,
        short = "p",
        value_name = "PACKAGE"
    )]
    /// Additional dependencies to be added to the environment
    packages: Vec<String>,
}

/// Possible subcommands
#[derive(Clap, Deserialize, Serialize)]
pub enum SubCommand {
    /// `shell` subcommand specified
    #[clap(name = "shell", about = "Spawn an additional shell session")]
    Shell,
}

impl Default for Config {
    fn default() -> Self {
        Self::parse()
    }
}

impl Config {
    const FILENAME: &'static str = "kubernix.toml";

    /// Make the configs root path absolute
    pub fn canonicalize_root(&mut self) -> Fallible<()> {
        self.create_root_dir()?;
        self.root = canonicalize(self.root())
            .map_err(|e| format_err!("Unable to canonicalize config root directory: {}", e))?;
        Ok(())
    }

    /// Write the current configuration to the internal set root path
    pub fn to_file(&self) -> Fallible<()> {
        self.create_root_dir()?;
        fs::write(self.root().join(Self::FILENAME), toml::to_string(&self)?)
            .map_err(|e| format_err!("Unable to write configuration to file: {}", e))?;
        Ok(())
    }

    /// Read the configuration from the internal set root path
    pub fn update_from_file(&mut self) -> Fallible<()> {
        let file = self.root().join(Self::FILENAME);
        *self = toml::from_str(&read_to_string(&file).map_err(|e| {
            format_err!(
                "Unable to read expected configuration file '{}': {}",
                file.display(),
                e
            )
        })?)
        .map_err(|e| format_err!("Unable to load config file '{}': {}", file.display(), e))?;
        Ok(())
    }

    fn create_root_dir(&self) -> Fallible<()> {
        create_dir_all(self.root())
            .map_err(|e| format_err!("Unable to create root directory: {}", e))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    pub fn test_config() -> Fallible<Config> {
        let mut c = Config::default();
        c.root = tempdir()?.into_path();
        c.canonicalize_root()?;
        Ok(c)
    }

    pub fn test_config_wrong_root() -> Fallible<Config> {
        let mut c = test_config()?;
        c.root = Path::new("/").join("proc");
        Ok(c)
    }

    pub fn test_config_wrong_cidr() -> Fallible<Config> {
        let mut c = test_config()?;
        c.cidr = "10.0.0.1/25".parse()?;
        Ok(c)
    }

    #[test]
    fn canonicalize_root_success() -> Fallible<()> {
        let mut c = Config::default();
        c.root = tempdir()?.into_path();
        c.canonicalize_root()
    }

    #[test]
    fn canonicalize_root_failure() {
        let mut c = Config::default();
        c.root = Path::new("/").join("proc").join("invalid");
        assert!(c.canonicalize_root().is_err())
    }

    #[test]
    fn to_file_success() -> Fallible<()> {
        let mut c = Config::default();
        c.root = tempdir()?.into_path();
        c.to_file()
    }

    #[test]
    fn to_file_failure() {
        let mut c = Config::default();
        c.root = Path::new("/").join("proc").join("invalid");
        assert!(c.to_file().is_err())
    }

    #[test]
    fn update_from_file_success() -> Fallible<()> {
        let mut c = Config::default();
        c.root = tempdir()?.into_path();
        fs::write(
            c.root.join(Config::FILENAME),
            r#"
root = "root"
log-level = "DEBUG"
cidr = "1.1.1.1/16"
impure = false
packages = []
            "#,
        )?;
        c.update_from_file()?;
        assert_eq!(c.root(), Path::new("root"));
        assert_eq!(c.log_level(), &LevelFilter::Debug);
        assert_eq!(c.cidr().to_string(), "1.1.1.1/16");
        Ok(())
    }

    #[test]
    fn update_from_file_failure() -> Fallible<()> {
        let mut c = Config::default();
        c.root = tempdir()?.into_path();
        fs::write(c.root.join(Config::FILENAME), "invalid")?;
        assert!(c.update_from_file().is_err());
        Ok(())
    }
}
