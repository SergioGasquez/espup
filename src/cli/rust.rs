use crate::{
    config::Config,
    emoji,
    error::Error,
    export_file::{export_environment, get_export_file},
    host_triple::get_host_triple,
    logging::initialize_logger,
    targets::{parse_targets, Target},
    toolchain::{
        esp_idf::{get_dist_path, EspIdfRepo},
        gcc::Gcc,
        llvm::Llvm,
        rust::{check_rust_installation, Crate, RiscVTarget, XtensaRust},
        Installable,
    },
    update::check_for_update,
};
use clap::Parser;
use log::{debug, info, warn};
use miette::Result;
use std::{
    collections::HashSet,
    fs::{remove_dir_all, remove_file},
    path::PathBuf,
};
use tokio::sync::mpsc;

#[derive(Debug, Parser)]
pub struct InstallOpts {
    /// Target triple of the host.
    #[arg(short = 'd', long, required = false, value_parser = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu" , "x86_64-apple-darwin" , "aarch64-apple-darwin"])]
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
    #[arg(short = 'd', long, required = false, value_parser = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu" , "x86_64-apple-darwin" , "aarch64-apple-darwin"])]
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
pub async fn install(args: InstallOpts) -> Result<()> {
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
            let latest_version = XtensaRust::get_latest_version().await?;
            XtensaRust::new(&latest_version, &host_triple)
        };
        Some(xtensa_rust)
    } else {
        None
    };
    let export_file = get_export_file(args.export_file)?;
    let llvm = Llvm::new(args.llvm_version, args.profile_minimal, &host_triple);
    let llvm_path = Some(llvm.path.clone());

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
        &llvm,
        &args.nightly_version,
        xtensa_rust,
        args.profile_minimal,
        args.toolchain_version,
    );

    #[cfg(windows)]
    check_arguments(&targets, &args.esp_idf_version)?;

    check_rust_installation(&args.nightly_version, &host_triple).await?;

    // Build up a vector of installable applications, all of which implement the
    // `Installable` async trait.
    let mut to_install = Vec::<Box<dyn Installable + Send + Sync>>::new();

    if let Some(ref xtensa_rust) = xtensa_rust {
        to_install.push(Box::new(xtensa_rust.to_owned()));
    }

    to_install.push(Box::new(llvm));

    if targets.iter().any(|t| t.riscv()) {
        let riscv_target = RiscVTarget::new(&args.nightly_version);
        to_install.push(Box::new(riscv_target));
    }

    if let Some(esp_idf_version) = &args.esp_idf_version {
        let repo = EspIdfRepo::new(esp_idf_version, args.profile_minimal, &targets);
        to_install.push(Box::new(repo));
        if let Some(ref mut extra_crates) = extra_crates {
            extra_crates.insert(Crate::new("ldproxy"));
        } else {
            let mut crates = HashSet::new();
            crates.insert(Crate::new("ldproxy"));
            extra_crates = Some(crates);
        };
    } else {
        targets.iter().for_each(|target| {
            if target.xtensa() {
                let gcc = Gcc::new(target, &host_triple);
                to_install.push(Box::new(gcc));
            }
        });
        // All RISC-V targets use the same GCC toolchain
        // ESP32S2 and ESP32S3 also install the RISC-V toolchain for their ULP coprocessor
        if targets.iter().any(|t| t != &Target::ESP32) {
            let riscv_gcc = Gcc::new_riscv(&host_triple);
            to_install.push(Box::new(riscv_gcc));
        }
    }

    if let Some(ref extra_crates) = &extra_crates {
        for extra_crate in extra_crates {
            to_install.push(Box::new(extra_crate.to_owned()));
        }
    }

    // With a list of applications to install, install them all in parallel.
    let (tx, mut rx) = mpsc::channel::<Result<Vec<String>, Error>>(32);
    let installable_items = to_install.len();
    for app in to_install {
        let tx = tx.clone();
        tokio::spawn(async move {
            let res = app.install().await;
            tx.send(res).await.unwrap();
        });
    }

    // Read the results of the install tasks as they complete.
    for _ in 0..installable_items {
        let names = rx.recv().await.unwrap()?;
        exports.extend(names);
    }

    if args.profile_minimal {
        clear_dist_folder()?;
    }

    export_environment(&export_file, &exports)?;

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
        llvm_path,
        nightly_version: args.nightly_version,
        targets,
        xtensa_rust,
    };
    info!("{} Saving configuration file", emoji::WRENCH);
    config.save()?;

    info!("{} Installation successfully completed!", emoji::CHECK);
    warn!(
        "{} Please, source the export file, as state above, to properly setup the environment!",
        emoji::WARN
    );
    Ok(())
}

/// Uninstalls the Rust for ESP chips environment
pub async fn uninstall(args: UninstallOpts) -> Result<()> {
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
        config.xtensa_rust = None;
        config.save()?;
        xtensa_rust.uninstall()?;
    }

    if let Some(llvm_path) = config.llvm_path {
        let llvm_path = llvm_path.parent().unwrap();
        config.llvm_path = None;
        config.save()?;
        Llvm::uninstall(llvm_path)?;
    }

    if config.targets.iter().any(|t| t.riscv()) {
        RiscVTarget::uninstall(&config.nightly_version)?;
    }

    if let Some(esp_idf_version) = config.esp_idf_version {
        config.esp_idf_version = None;
        config.save()?;
        EspIdfRepo::uninstall(&esp_idf_version)?;
    } else {
        info!("{} Deleting GCC targets", emoji::WRENCH);
        if config.targets.iter().any(|t| t != &Target::ESP32) {
            // All RISC-V targets use the same GCC toolchain
            // ESP32S2 and ESP32S3 also install the RISC-V toolchain for their ULP coprocessor
            config.targets.remove(&Target::ESP32C3);
            config.targets.remove(&Target::ESP32C2);
            config.save()?;
            Gcc::uninstall_riscv()?;
        }
        for target in &config.targets.clone() {
            if target.xtensa() {
                config.targets.remove(target);
                config.save()?;
                Gcc::uninstall(target)?;
            }
        }
    }

    if config.extra_crates.is_some() {
        info!("{} Uninstalling extra crates", emoji::WRENCH);
        let mut updated_extra_crates: HashSet<String> = config.extra_crates.clone().unwrap();
        for extra_crate in &config.extra_crates.clone().unwrap() {
            updated_extra_crates.remove(extra_crate);
            config.extra_crates = Some(updated_extra_crates.clone());
            config.save()?;
            Crate::uninstall(extra_crate)?;
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
    Config::delete()?;

    info!("{} Uninstallation successfully completed!", emoji::CHECK);
    Ok(())
}

/// Updates Xtensa Rust toolchain.
pub async fn update(args: UpdateOpts) -> Result<()> {
    initialize_logger(&args.log_level);
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!("{} Updating ESP Rust environment", emoji::DISC);
    let host_triple = get_host_triple(args.default_host)?;
    let mut config = Config::load()?;
    let xtensa_rust: XtensaRust = if let Some(toolchain_version) = args.toolchain_version {
        XtensaRust::new(&toolchain_version, &host_triple)
    } else {
        let latest_version = XtensaRust::get_latest_version().await?;
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
        xtensa_rust.install().await?;
        config.xtensa_rust = Some(xtensa_rust);
    }

    config.save()?;

    info!("{} Update successfully completed!", emoji::CHECK);
    Ok(())
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