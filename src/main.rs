use clap::Parser;
use dirs::home_dir;
use embuild::{
    cmd,
    espidf::{parse_esp_idf_git_ref, EspIdfRemote},
};
use espup::{
    config::Config,
    emoji,
    error::Error,
    host_triple::get_host_triple,
    logging::initialize_logger,
    targets::{parse_targets, Target},
    toolchain::{
        espidf::{
            get_dist_path, get_install_path, get_tool_path, EspIdfRepo, DEFAULT_GIT_REPOSITORY,
        },
        gcc::{get_toolchain_name, install_gcc_targets},
        llvm::Llvm,
        rust::{
            check_rust_installation, install_extra_crates, install_riscv_target, Crate, XtensaRust,
        },
    },
    update::check_for_update,
};
use log::{debug, info, warn};
use miette::{IntoDiagnostic, Result};
use std::{
    collections::HashSet,
    fs::{remove_dir_all, remove_file, File},
    io::Write,
    path::PathBuf,
};

#[cfg(feature = "gui")]
slint::include_modules!();

#[cfg(windows)]
const DEFAULT_EXPORT_FILE: &str = "export-esp.ps1";
#[cfg(not(windows))]
const DEFAULT_EXPORT_FILE: &str = "export-esp.sh";

#[derive(Parser)]
#[command(
    name = "espup",
    bin_name = "espup",
    version,
    propagate_version = true,
    about,
    arg_required_else_help(true)
)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
pub enum SubCommand {
    /// Installs esp-rs environment
    Install(Box<InstallOpts>),
    #[cfg(feature = "gui")]
    /// GUI to install/update/uninstall the Rust for ESP chips environment
    Gui,
    /// Uninstalls esp-rs environment
    Uninstall(UninstallOpts),
    /// Updates Xtensa Rust toolchain
    Update(UpdateOpts),
}

#[derive(Debug, Parser)]
pub struct InstallOpts {
    /// Target triple of the host.
    #[arg(short = 'd', long, required = false)]
    pub default_host: Option<String>,
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
    #[arg(short = 'e', long, required = false)]
    pub esp_idf_version: Option<String>,
    /// Destination of the generated export file.
    #[arg(short = 'f', long)]
    pub export_file: Option<PathBuf>,
    /// Comma or space list of extra crates to install.
    #[arg(short = 'c', long, required = false, value_parser = Crate::parse_crates)]
    pub extra_crates: Option<HashSet<Crate>>,
    /// LLVM version.
    #[arg(short = 'x', long, default_value = "15", value_parser = ["15"])]
    pub llvm_version: String,
    /// Verbosity level of the logs.
    #[arg(short = 'l', long, default_value = "info", value_parser = ["debug", "info", "warn", "error"])]
    pub log_level: String,
    /// Nightly Rust toolchain version.
    #[arg(short = 'n', long, default_value = "nightly")]
    pub nightly_version: String,
    ///  Minifies the installation.
    #[arg(short = 'm', long)]
    pub profile_minimal: bool,
    /// Comma or space separated list of targets [esp32,esp32s2,esp32s3,esp32c2,esp32c3,all].
    #[arg(short = 't', long, default_value = "all", value_parser = parse_targets)]
    pub targets: HashSet<Target>,
    /// Xtensa Rust toolchain version.
    #[arg(short = 'v', long, value_parser = XtensaRust::parse_version)]
    pub toolchain_version: Option<String>,
}

#[derive(Debug, Parser)]
pub struct UpdateOpts {
    /// Target triple of the host.
    #[arg(short = 'd', long, required = false)]
    pub default_host: Option<String>,
    /// Verbosity level of the logs.
    #[arg(short = 'l', long, default_value = "info", value_parser = ["debug", "info", "warn", "error"])]
    pub log_level: String,
    /// Xtensa Rust toolchain version.
    #[arg(short = 'v', long, value_parser = XtensaRust::parse_version)]
    pub toolchain_version: Option<String>,
}

#[derive(Debug, Parser)]
pub struct UninstallOpts {
    /// Verbosity level of the logs.
    #[arg(short = 'l', long, default_value = "info", value_parser = ["debug", "info", "warn", "error"])]
    pub log_level: String,
}

/// Installs the Rust for ESP chips environment
fn install(args: InstallOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    info!("{} Installing esp-rs", emoji::DISC);
    let targets = args.targets;
    let host_triple = get_host_triple(args.default_host)?;
    let mut extra_crates = args.extra_crates;
    let mut exports: Vec<String> = Vec::new();
    let xtensa_rust = if targets.contains(&Target::ESP32)
        || targets.contains(&Target::ESP32S2)
        || targets.contains(&Target::ESP32S3)
    {
        let xtensa_rust: XtensaRust = if let Some(toolchain_version) = &args.toolchain_version {
            XtensaRust::new(toolchain_version, &host_triple)
        } else {
            let latest_version = XtensaRust::get_latest_version()?;
            XtensaRust::new(&latest_version, &host_triple)
        };
        Some(xtensa_rust)
    } else {
        None
    };
    let export_file = get_export_file(args.export_file)?;
    let llvm = Llvm::new(args.llvm_version, args.profile_minimal, &host_triple);

    debug!(
        "{} Arguments:
            - Host triple: {}
            - Targets: {:?}
            - ESP-IDF version: {:?}
            - Export file: {:?}
            - Extra crates: {:?}
            - LLVM Toolchain: {:?}
            - Nightly version: {:?}
            - Rust Toolchain: {:?}
            - Profile Minimal: {:?}
            - Toolchain version: {:?}",
        emoji::INFO,
        host_triple,
        targets,
        &args.esp_idf_version,
        &export_file,
        &extra_crates,
        llvm,
        &args.nightly_version,
        xtensa_rust,
        args.profile_minimal,
        args.toolchain_version,
    );

    #[cfg(windows)]
    check_arguments(&targets, &args.esp_idf_version)?;

    check_rust_installation(&args.nightly_version, &host_triple)?;

    if let Some(ref xtensa_rust) = xtensa_rust {
        xtensa_rust.install()?;
    }

    exports.extend(llvm.install()?);

    if targets.contains(&Target::ESP32C3) {
        install_riscv_target(&args.nightly_version)?;
    }

    if let Some(esp_idf_version) = &args.esp_idf_version {
        let repo = EspIdfRepo::new(esp_idf_version, args.profile_minimal, &targets);
        exports.extend(repo.install()?);
        if let Some(ref mut extra_crates) = extra_crates {
            extra_crates.insert(Crate::new("ldproxy"));
        } else {
            let mut crates = HashSet::new();
            crates.insert(Crate::new("ldproxy"));
            extra_crates = Some(crates);
        };
    } else {
        exports.extend(install_gcc_targets(&targets, &host_triple)?);
    }

    if let Some(ref extra_crates) = &extra_crates {
        install_extra_crates(extra_crates)?;
    }

    if args.profile_minimal {
        clear_dist_folder()?;
    }

    export_environment(&export_file, &exports)?;

    info!("{} Saving configuration file", emoji::WRENCH);
    let config = Config {
        esp_idf_version: args.esp_idf_version,
        export_file: Some(export_file),
        extra_crates: extra_crates.as_ref().map(|extra_crates| {
            extra_crates
                .iter()
                .map(|x| x.name.clone())
                .collect::<HashSet<String>>()
        }),
        host_triple,
        llvm_path: Some(llvm.path),
        nightly_version: args.nightly_version,
        targets,
        xtensa_rust,
    };
    config.save()?;

    info!("{} Installation successfully completed!", emoji::CHECK);
    warn!(
        "{} Please, source the export file, as state above, to properly setup the environment!",
        emoji::WARN
    );
    Ok(())
}

#[cfg(feature = "gui")]
/// GUI to install/update/uninstall the Rust for ESP chips environment
fn gui() -> Result<()> {
    let app = App::new();
    let host_triple = get_host_triple(None)?;
    let latest_xtensa_rust = XtensaRust::get_latest_version()?;
    let export_file = get_export_file(None)?;
    // Set defaults
    app.global::<Args>()
        .set_xtensa_rust_version(latest_xtensa_rust.into());
    app.global::<Args>()
        .set_default_host(host_triple.to_string().into());
    app.global::<Args>()
        .set_export_file(export_file.display().to_string().into());

    if Config::get_config_path().unwrap().exists() {
        app.global::<Args>().set_uninstall_enable(true);
        app.global::<Args>().set_button("Update".into());
        let config = Config::load()?;
        if let Some(xtensa_rust) = config.xtensa_rust {
            app.global::<Args>()
                .set_xtensa_rust_version(xtensa_rust.version.into());
        }
        app.global::<Args>()
            .set_default_host(config.host_triple.to_string().into());
        app.global::<Args>()
            .set_nightly_version(config.nightly_version.into());
        if let Some(export_file) = config.export_file {
            app.global::<Args>()
                .set_export_file(export_file.display().to_string().into());
        }
        if let Some(esp_idf_version) = config.esp_idf_version {
            app.global::<Args>()
                .set_esp_idf_version(esp_idf_version.into());
        }
        if !config.targets.contains(&Target::ESP32) {
            app.global::<Args>().set_esp32_value(false);
        }
        if !config.targets.contains(&Target::ESP32S2) {
            app.global::<Args>().set_esp32s2_value(false);
        }
        if !config.targets.contains(&Target::ESP32S3) {
            app.global::<Args>().set_esp32s3_value(false);
        }
        if !config.targets.contains(&Target::ESP32C3) {
            app.global::<Args>().set_esp32c3_value(false);
        }
        if !config.targets.contains(&Target::ESP32C2) {
            app.global::<Args>().set_esp32c2_value(false);
        }
        if let Some(extra_crates) = config.extra_crates {
            if extra_crates.contains("espflash") {
                app.global::<Args>().set_espflash_value(true);
            }
            if extra_crates.contains("cargo-espflash") {
                app.global::<Args>().set_cargo_espflash_value(true);
            }
            if extra_crates.contains("cargo-generate") {
                app.global::<Args>().set_cargo_generate_value(true);
            }
            if extra_crates.contains("ldproxy") {
                app.global::<Args>().set_ldproxy_value(true);
            }
            if extra_crates.contains("sccache") {
                app.global::<Args>().set_sccache_value(true);
            }
        }
        app.global::<Args>().set_install_mode(false);
    }

    // Install/Update callback
    app.global::<Args>().on_install({
        let ui_handle = app.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            let mut selected_crates: HashSet<Crate> = HashSet::new();
            let mut targets: HashSet<Target> = HashSet::new();
            // Get targets
            if ui.global::<Args>().get_esp32_value() {
                targets.insert(Target::ESP32);
            }
            if ui.global::<Args>().get_esp32s2_value() {
                targets.insert(Target::ESP32S2);
            }
            if ui.global::<Args>().get_esp32s3_value() {
                targets.insert(Target::ESP32S3);
            }
            if ui.global::<Args>().get_esp32c2_value() {
                targets.insert(Target::ESP32C2);
            }
            if ui.global::<Args>().get_esp32c3_value() {
                targets.insert(Target::ESP32C3);
            }
            // Get extra crates
            if ui.global::<Args>().get_espflash_value() {
                selected_crates.insert(Crate::new("espflash"));
            }
            if ui.global::<Args>().get_cargo_espflash_value() {
                selected_crates.insert(Crate::new("cargo-espflash"));
            }
            if ui.global::<Args>().get_cargo_generate_value() {
                selected_crates.insert(Crate::new("cargo-generate"));
            }
            if ui.global::<Args>().get_ldproxy_value() {
                selected_crates.insert(Crate::new("ldproxy"));
            }
            if ui.global::<Args>().get_sccache_value() {
                selected_crates.insert(Crate::new("sccache"));
            }
            let extra_crates = if selected_crates.is_empty() {
                None
            } else {
                Some(selected_crates)
            };
            // Host triple
            let host_triple = ui.global::<Args>().get_default_host();
            // Log Level
            let log_level = ui.global::<Args>().get_log_level().to_string();
            // Export file
            let export_file = ui.global::<Args>().get_export_file();
            let export_file = Some(PathBuf::from(export_file.as_str()));
            // ESP-IDF version
            let esp_idf_version = if (ui.global::<Args>().get_esp_idf_version()) == "none" {
                None
            } else {
                Some(ui.global::<Args>().get_esp_idf_version().to_string())
            };
            // Xtensa Rust Toolhain version
            let xtensa_rust_version = ui.global::<Args>().get_xtensa_rust_version().to_string();
            // Nightly Rust Toolhain version
            let nightly_version = ui.global::<Args>().get_nightly_version().to_string();
            let profile_minimal = ui.global::<Args>().get_profile_minimal();
            if ui.global::<Args>().get_button() == "Install" {
                let opts = InstallOpts {
                    default_host: Some(host_triple.into()),
                    esp_idf_version,
                    export_file,
                    extra_crates,
                    llvm_version: "15".into(),
                    log_level,
                    nightly_version,
                    profile_minimal,
                    targets: targets.clone(),
                    toolchain_version: Some(xtensa_rust_version),
                };
                if install(opts).is_err() {
                    panic!("Installation failed");
                }
            } else {
                let opts = UpdateOpts {
                    default_host: Some(host_triple.into()),
                    log_level,
                    toolchain_version: Some(xtensa_rust_version),
                };
                if update(opts).is_err() {
                    panic!("Update failed");
                };
            }
        }
    });
    // Uninstall callback
    app.global::<Args>().on_uninstall({
        let ui_handle = app.as_weak();
        move || {
            let ui = ui_handle.unwrap();

            // Log Level
            let log_level = ui.global::<Args>().get_log_level().to_string();

            let opts = UninstallOpts { log_level };
            println!("Uninstall options: {:#?}", opts);
            if uninstall(opts).is_err() {
                panic!("Uninstall failed");
            };
        }
    });
    app.run();
    Ok(())
}

/// Uninstalls the Rust for ESP chips environment
fn uninstall(args: UninstallOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!("{} Uninstalling esp-rs", emoji::DISC);
    let mut config = Config::load()?;

    debug!(
        "{} Arguments:
            - Config: {:#?}",
        emoji::INFO,
        config
    );

    if let Some(xtensa_rust) = config.xtensa_rust {
        info!("{} Deleting Xtensa Rust toolchain", emoji::WRENCH);
        config.xtensa_rust = None;
        config.save()?;
        xtensa_rust.uninstall()?;
    }

    if let Some(llvm_path) = config.llvm_path {
        info!("{} Deleting Xtensa LLVM", emoji::WRENCH);
        config.llvm_path = None;
        config.save()?;
        remove_dir_all(&llvm_path)
            .map_err(|_| Error::FailedToRemoveDirectory(llvm_path.display().to_string()))?;
    }

    if let Some(esp_idf_version) = config.esp_idf_version {
        info!("{} Deleting ESP-IDF {}", emoji::WRENCH, esp_idf_version);
        config.esp_idf_version = None;
        config.save()?;
        let repo = EspIdfRemote {
            git_ref: parse_esp_idf_git_ref(&esp_idf_version),
            repo_url: Some(DEFAULT_GIT_REPOSITORY.to_string()),
        };

        remove_dir_all(get_install_path(repo.clone()).parent().unwrap()).map_err(|_| {
            Error::FailedToRemoveDirectory(
                get_install_path(repo)
                    .parent()
                    .unwrap()
                    .display()
                    .to_string(),
            )
        })?;
    } else {
        info!("{} Deleting GCC targets", emoji::WRENCH);
        for target in &config.targets.clone() {
            config.targets.remove(target);
            config.save()?;
            let gcc_path = get_tool_path(&get_toolchain_name(target));
            remove_dir_all(&gcc_path).map_err(|_| Error::FailedToRemoveDirectory(gcc_path))?;
        }
    }

    if config.extra_crates.is_some() {
        info!("{} Uninstalling extra crates", emoji::WRENCH);
        let mut updated_extra_crates: HashSet<String> = config.extra_crates.clone().unwrap();
        for extra_crate in &config.extra_crates.clone().unwrap() {
            updated_extra_crates.remove(extra_crate);
            config.extra_crates = Some(updated_extra_crates.clone());
            config.save()?;
            cmd!("cargo", "uninstall", extra_crate)
                .run()
                .into_diagnostic()?;
        }
    }

    if let Some(export_file) = config.export_file {
        info!("{} Deleting export file", emoji::WRENCH);
        config.export_file = None;
        config.save()?;
        remove_file(&export_file)
            .map_err(|_| Error::FailedToRemoveFile(export_file.display().to_string()))?;
    }

    clear_dist_folder()?;
    info!("{} Deleting config file", emoji::WRENCH);
    let conf_file = Config::get_config_path()?;
    remove_file(&conf_file)
        .map_err(|_| Error::FailedToRemoveFile(conf_file.display().to_string()))?;

    info!("{} Uninstallation successfully completed!", emoji::CHECK);
    Ok(())
}

/// Updates Xtensa Rust toolchain.
fn update(args: UpdateOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!("{} Updating ESP Rust environment", emoji::DISC);
    let host_triple = get_host_triple(args.default_host)?;
    let mut config = Config::load()?;
    let xtensa_rust: XtensaRust = if let Some(toolchain_version) = args.toolchain_version {
        XtensaRust::new(&toolchain_version, &host_triple)
    } else {
        let latest_version = XtensaRust::get_latest_version()?;
        XtensaRust::new(&latest_version, &host_triple)
    };

    debug!(
        "{} Arguments:
            - Host triple: {}
            - Toolchain version: {:#?}
            - Config: {:#?}",
        emoji::INFO,
        host_triple,
        xtensa_rust,
        config
    );

    if let Some(config_xtensa_rust) = config.xtensa_rust {
        if config_xtensa_rust.version == xtensa_rust.version {
            info!(
                "{} Toolchain '{}' is already up to date",
                emoji::CHECK,
                xtensa_rust.version
            );
            return Ok(());
        }
        config_xtensa_rust.uninstall()?;
        xtensa_rust.install()?;
        config.xtensa_rust = Some(xtensa_rust);
    }

    config.save()?;

    info!("{} Update successfully completed!", emoji::CHECK);
    Ok(())
}

fn main() -> Result<()> {
    match Cli::parse().subcommand {
        SubCommand::Install(args) => install(*args),
        #[cfg(feature = "gui")]
        SubCommand::Gui => gui(),
        SubCommand::Update(args) => update(args),
        SubCommand::Uninstall(args) => uninstall(args),
    }
}

/// Deletes dist folder.
fn clear_dist_folder() -> Result<(), Error> {
    let dist_path = PathBuf::from(get_dist_path(""));
    if dist_path.exists() {
        info!("{} Clearing dist folder", emoji::WRENCH);
        remove_dir_all(&dist_path)
            .map_err(|_| Error::FailedToRemoveDirectory(dist_path.display().to_string()))?;
    }
    Ok(())
}

/// Returns the absolute path to the export file, uses the DEFAULT_EXPORT_FILE if no arg is provided.
fn get_export_file(export_file: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(export_file) = export_file {
        if export_file.is_absolute() {
            Ok(export_file)
        } else {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(export_file))
        }
    } else {
        let home_dir = home_dir().unwrap();
        Ok(home_dir.join(DEFAULT_EXPORT_FILE))
    }
}

/// Creates the export file with the necessary environment variables.
fn export_environment(export_file: &PathBuf, exports: &[String]) -> Result<(), Error> {
    info!("{} Creating export file", emoji::WRENCH);
    let mut file = File::create(export_file)?;
    for e in exports.iter() {
        file.write_all(e.as_bytes())?;
        file.write_all(b"\n")?;
    }
    #[cfg(windows)]
    warn!(
        "{} PLEASE set up the environment variables running: '{}'",
        emoji::INFO,
        export_file.display()
    );
    #[cfg(unix)]
    warn!(
        "{} PLEASE set up the environment variables running: '. {}'",
        emoji::INFO,
        export_file.display()
    );
    warn!(
        "{} This step must be done every time you open a new terminal.",
        emoji::WARN
    );
    Ok(())
}

#[cfg(windows)]
/// For Windows, we need to check that we are installing all the targets if we are installing esp-idf.
pub fn check_arguments(
    targets: &HashSet<Target>,
    espidf_version: &Option<String>,
) -> Result<(), Error> {
    if espidf_version.is_some()
        && (!targets.contains(&Target::ESP32)
            || !targets.contains(&Target::ESP32C3)
            || !targets.contains(&Target::ESP32S2)
            || !targets.contains(&Target::ESP32S3))
    {
        return Err(Error::WrongWindowsArguments);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{get_export_file, DEFAULT_EXPORT_FILE};
    use dirs::home_dir;
    use std::{env::current_dir, path::PathBuf};

    #[test]
    #[allow(unused_variables)]
    fn test_get_export_file() {
        // No arg provided
        let home_dir = home_dir().unwrap();
        let export_file = home_dir.join(DEFAULT_EXPORT_FILE);
        assert!(matches!(get_export_file(None), Ok(export_file)));
        // Relative path
        let current_dir = current_dir().unwrap();
        let export_file = current_dir.join("export.sh");
        assert!(matches!(
            get_export_file(Some(PathBuf::from("export.sh"))),
            Ok(export_file)
        ));
        // Absolute path
        let export_file = PathBuf::from("/home/user/export.sh");
        assert!(matches!(
            get_export_file(Some(PathBuf::from("/home/user/export.sh"))),
            Ok(export_file)
        ));
    }
}
