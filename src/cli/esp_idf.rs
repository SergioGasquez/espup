use crate::{
    emoji,
    logging::initialize_logger,
    targets::{parse_targets, Target},
    toolchain::{esp_idf::EspIdfRepo, Installable},
    update::check_for_update,
};
use clap::Parser;
use log::info;
use miette::Result;
use std::collections::HashSet;

#[derive(Debug, Parser)]
pub struct InstallOpts {
    /// ESP-IDF version to install. If empty, no esp-idf is installed. Version format:
    ///
    /// - `commit:<hash>`: Uses the commit `<hash>` of the `esp-idf` repository.
    ///
    /// - `tag:<tag>`: Uses the tag `<tag>` of the `esp-idf` repository.
    ///
    /// - `branch:<branch>`: Uses the branch `<branch>` of the `esp-idf` repository.
    ///
    /// - `v<major>.<minor>` or `<major>.<minor>`: Uses the tag `v<major>.<minor>` of the `esp-idf` repository.
    ///
    /// - `<branch>`: Uses the branch `<branch>` of the `esp-idf` repository.
    ///
    /// When using this option, `ldproxy` crate will also be installed.
    #[arg(short = 'e', long, required = true)]
    pub esp_idf_version: String,
    /// Verbosity level of the logs.
    #[arg(short = 'l', long, default_value = "info", value_parser = ["debug", "info", "warn", "error"])]
    pub log_level: String,
    /// Comma or space separated list of targets [esp32,esp32s2,esp32s3,esp32c2,esp32c3,all].
    #[arg(short = 't', long, default_value = "all", value_parser = parse_targets)]
    pub targets: HashSet<Target>,
}

/// Installs the Rust for ESP chips environment
pub async fn install(args: InstallOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    info!("{} Installing ESP-IDF", emoji::DISC);
    let targets = args.targets;
    let repo = EspIdfRepo::new(&args.esp_idf_version, false, &targets);
    repo.install().await?;
    // Update config file
    Ok(())
}
