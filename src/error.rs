use crate::emoji;

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
pub enum Error {
    // Host Triple
    #[diagnostic(code(espup::host_triple::unsupported_host_triple))]
    #[error("{} Host triple '{0}' is not supported", emoji::ERROR)]
    UnsupportedHostTriple(String),
    // Target
    #[diagnostic(code(espup::targets::unsupported_target))]
    #[error("{} Target '{0}' is not supported", emoji::ERROR)]
    UnsupportedTarget(String),
    //  Config
    #[diagnostic(code(espup::config::file_not_found))]
    #[error("{} No config file found in '{0}'", emoji::ERROR)]
    FileNotFound(String),
    #[diagnostic(code(espup::config::failed_to_deserialize))]
    #[error("{} Failed to deserialize config", emoji::ERROR)]
    FailedToDeserialize,
    #[diagnostic(code(espup::config::failed_to_serialize))]
    #[error("{} Failed to serialize config", emoji::ERROR)]
    FailedToSerialize,
    #[diagnostic(code(espup::config::failed_to_create_config_file))]
    #[error("{} Failed to create config directory", emoji::ERROR)]
    FailedToCreateConfigFile,
    #[diagnostic(code(espup::config::failed_to_write))]
    #[error("{} Failed to write config to '{0}'", emoji::ERROR)]
    FailedToWrite(String),
    //  Toolchain
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    RewquestError(#[from] reqwest::Error),
    #[diagnostic(code(espup::toolchain::failed_to_create_directory))]
    #[error("{} Creating directory '{0}' failed", emoji::ERROR)]
    FailedToCreateDirectory(String),
    #[diagnostic(code(espup::toolchain::unsupported_file_extension))]
    #[error("{} Unsuported file extension: '{0}'", emoji::ERROR)]
    UnsuportedFileExtension(String),
    //  Toolchain - Rust
    #[diagnostic(code(espup::toolchain::rust::failed_to_get_latest_version))]
    #[error("{} Failed To serialize Json from string.", emoji::ERROR)]
    FailedToSerializeJson,
    #[diagnostic(code(espup::toolchain::rust::xtensa_rust_already_installed))]
    #[error("{} Previous installation of Rust Toolchain exists in: '{0}'. Please, remove the directory before new installation.", emoji::ERROR)]
    XtensaToolchainAlreadyInstalled(String),
    #[diagnostic(code(espup::toolchain::rust::invalid_version))]
    #[error(
        "{} Invalid toolchain version '{0}', must be in the form of '<major>.<minor>.<patch>.<subpatch>'",
        emoji::ERROR
    )]
    InvalidXtensaToolchanVersion(String),
    #[diagnostic(code(espup::toolchain::rust::detection_error))]
    #[error("{} Error detecting rustup: {0}", emoji::ERROR)]
    RustupDetectionError(String),
    #[error(transparent)]
    CmdError(#[from] embuild::cmd::CmdError),
    // Toolchain - ESP-IDF
    #[diagnostic(code(espup::toolchain::esp_idf::failed_to_instatiate_cmake))]
    #[error("{} Failed to add CMake to ESP-IDF tools", emoji::ERROR)]
    FailedToInstantiateCmake,
    #[diagnostic(code(espup::toolchain::esp_idf::failed_to_create_esp_idf_install_closure))]
    #[error("{} Failed to create ESP-IDF  install closure", emoji::ERROR)]
    FailedToCreateEspIdfInstallClosure,
    #[diagnostic(code(espup::toolchain::esp_idf::failed_to_install_esp_idf))]
    #[error("{} Failed to install ESP-IDF. Please, manually verify that '{0}' is a proper ESP-IDF version.", emoji::ERROR)]
    FailedToInstallEspIdf(String),
    //  Main
    #[diagnostic(code(espup::wrong_windows_arguments))]
    #[error(
        "{} When installing esp-idf in Windows, only --targets \"all\" is supported.",
        emoji::ERROR
    )]
    WrongWindowsArguments,
    #[diagnostic(code(espup::failed_to_remove_directory))]
    #[error(
        "{} Failed to remove '{0}' directory. Please, manually verify that the directory is properly removed and run 'espup uninstall' again.",
        emoji::ERROR
    )]
    FailedToRemoveDirectory(String),
    #[diagnostic(code(espup::failed_to_remove_file))]
    #[error(
        "{} Failed to remove '{0}' file. Please, manually verify that the file is properly removed.",
        emoji::ERROR
    )]
    FailedToRemoveFile(String),
}
