use clap::Parser;
use espup::cli::{esp_idf, rust, Cli, EspIdf, Rust, SubCommand};
use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().subcommand {
        SubCommand::Rust(rust) => match rust {
            Rust::Install(args) => rust::install(*args).await,
            Rust::Update(args) => rust::update(args).await,
            Rust::Uninstall(args) => rust::uninstall(args).await,
        },
        SubCommand::EspIdf(esp_idf) => match esp_idf {
            EspIdf::Install(args) => esp_idf::install(args).await,
            // EspIdf::Uninstall => println!("Uninstall"),
        },
    }
}

#[cfg(windows)]
/// For Windows, we need to check that we are installing all the targets if we are installing esp-idf.
pub fn check_arguments(
    targets: &HashSet<Target>,
    esp_idf_version: &Option<String>,
) -> Result<(), Error> {
    if esp_idf_version.is_some()
        && (!targets.contains(&Target::ESP32)
            || !targets.contains(&Target::ESP32C3)
            || !targets.contains(&Target::ESP32S2)
            || !targets.contains(&Target::ESP32S3))
    {
        return Err(Error::WrongWindowsArguments);
    }

    Ok(())
}
