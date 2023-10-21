use clap::{CommandFactory, Parser};
use espup::{
    cli::{CompletionsOpts, InstallOpts, UninstallOpts},
    env::clean_env,
    error::Error,
    logging::initialize_logger,
    toolchain::{
        gcc::uninstall_gcc_toolchains, install as toolchain_install, llvm::Llvm,
        rust::get_rustup_home, InstallMode,
    },
    update::check_for_update,
};
use log::info;
use miette::Result;
use std::{env, fs::remove_dir_all};

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
pub enum SubCommand {
    /// Generate completions for the given shell.
    Completions(CompletionsOpts),
    /// Installs Espressif Rust ecosystem.
    // We use a Box here to make clippy happy (see https://rust-lang.github.io/rust-clippy/master/index.html#large_enum_variant)
    Install(Box<InstallOpts>),
    /// Uninstalls Espressif Rust ecosystem.
    Uninstall(UninstallOpts),
    /// Updates Xtensa Rust toolchain.
    Update(Box<InstallOpts>),
}

/// Updates Xtensa Rust toolchain.
async fn completions(args: CompletionsOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!("Generating completions for {} shell", args.shell);

    clap_complete::generate(
        args.shell,
        &mut Cli::command(),
        "espup",
        &mut std::io::stdout(),
    );

    info!("Completions successfully generated!");

    Ok(())
}

/// Installs or updates the Rust for ESP chips environment
async fn install(args: InstallOpts, install_mode: InstallMode) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    toolchain_install(args, install_mode).await?;
    Ok(())
}

/// Uninstalls the Rust for ESP chips environment
async fn uninstall(args: UninstallOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!("Uninstalling the Espressif Rust ecosystem");
    let install_dir = get_rustup_home().join("toolchains").join(args.name);

    Llvm::uninstall(&install_dir)?;

    uninstall_gcc_toolchains(&install_dir)?;

    info!(
        "Deleting the Xtensa Rust toolchain located in '{}'",
        &install_dir.display()
    );
    remove_dir_all(&install_dir)
        .map_err(|_| Error::RemoveDirectory(install_dir.display().to_string()))?;

    clean_env(&install_dir)?;

    info!("Uninstallation successfully completed!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().subcommand {
        SubCommand::Completions(args) => completions(args).await,
        SubCommand::Install(args) => install(*args, InstallMode::Install).await,
        SubCommand::Update(args) => install(*args, InstallMode::Update).await,
        SubCommand::Uninstall(args) => uninstall(args).await,
    }
}
