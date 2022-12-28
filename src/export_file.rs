use crate::{emoji, error::Error};
use dirs::home_dir;
use log::{info, warn};
use std::{
    io::Write,
    {fs::File, path::PathBuf},
};
#[cfg(windows)]
const DEFAULT_EXPORT_FILE: &str = "export-esp.ps1";
#[cfg(not(windows))]
const DEFAULT_EXPORT_FILE: &str = "export-esp.sh";

/// Returns the absolute path to the export file, uses the DEFAULT_EXPORT_FILE if no arg is provided.
pub fn get_export_file(export_file: Option<PathBuf>) -> Result<PathBuf, Error> {
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
pub fn export_environment(export_file: &PathBuf, exports: &[String]) -> Result<(), Error> {
    info!("{} Creating export file", emoji::WRENCH);
    let mut file = File::create(export_file)?;
    for e in exports.iter() {
        #[cfg(windows)]
        let e = e.replace('/', r#"\"#);
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
