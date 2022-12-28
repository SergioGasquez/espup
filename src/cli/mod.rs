use clap::Parser;

pub mod esp_idf;
pub mod rust;

#[derive(Parser)]
#[command(
    name = "espup",
    bin_name = "espup",
    version,
    propagate_version = true,
    about,
    arg_required_else_help(true)
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Parser)]
pub enum SubCommand {
    #[command(subcommand)]
    /// Configures the Rust environment for ESP chips
    Rust(Rust),
    #[command(subcommand)]
    /// Configures the ESP-IDF environment
    EspIdf(EspIdf),
}

#[derive(Parser)]
pub enum Rust {
    /// Installs the Rust environment for ESP chips
    Install(Box<rust::InstallOpts>),
    /// Uninstalls the Rust environment for ESP chips
    Uninstall(rust::UninstallOpts),
    /// Updates Xtensa Rust toolchain
    Update(rust::UpdateOpts),
}

#[derive(Parser)]
pub enum EspIdf {
    /// Installs an instance of ESP-IDF
    Install(esp_idf::InstallOpts),
}
