use std::io;
use std::path::Path;
use std::str::FromStr;

use url_vault_core::Vault;
use url_vault_native_host::{Browser, browser_manifest, install_browser_manifest, serve_bridge};

fn main() {
    if let Err(error) = run() {
        eprintln!("url-vault-native-host: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some("--print-browser-manifest") => {
            let browser = parse_browser(arguments.next())?;
            let host_path = arguments
                .next()
                .ok_or("--print-browser-manifest requires an absolute host path")?;
            if arguments.next().is_some() {
                return Err("--print-browser-manifest accepts exactly two arguments".into());
            }
            serde_json::to_writer_pretty(
                io::stdout().lock(),
                &browser_manifest(browser, Path::new(&host_path))?,
            )?;
            println!();
            return Ok(());
        }
        Some("--print-brave-manifest") => {
            let host_path = arguments
                .next()
                .ok_or("--print-brave-manifest requires an absolute host path")?;
            if arguments.next().is_some() {
                return Err("--print-brave-manifest accepts exactly one argument".into());
            }
            serde_json::to_writer_pretty(
                io::stdout().lock(),
                &browser_manifest(Browser::Brave, Path::new(&host_path))?,
            )?;
            println!();
            return Ok(());
        }
        Some("--install-browser") => {
            let browser = parse_browser(arguments.next())?;
            if arguments.next().is_some() {
                return Err("--install-browser accepts exactly one browser argument".into());
            }
            let host_path = std::env::current_exe()?;
            let extension_path = host_path
                .parent()
                .ok_or("native host executable has no parent directory")?
                .join("browser-extensions")
                .join(browser.as_str());
            if !extension_path.join("manifest.json").is_file() {
                return Err(format!(
                    "bundled {} extension not found: {}",
                    browser.display_name(),
                    extension_path.display()
                )
                .into());
            }
            let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
            let manifest_path = install_browser_manifest(browser, &host_path, Path::new(&home))?;
            serde_json::to_writer_pretty(
                io::stdout().lock(),
                &serde_json::json!({
                    "browser": browser,
                    "manifest_path": manifest_path,
                    "extension_path": extension_path,
                }),
            )?;
            println!();
            return Ok(());
        }
        _ => {}
    }

    let vault = Vault::from_env()?;
    vault.init()?;
    let stdin = std::fs::File::open("/dev/stdin")?;
    serve_bridge(stdin, io::stdout().lock(), &vault)?;
    Ok(())
}

fn parse_browser(value: Option<String>) -> Result<Browser, Box<dyn std::error::Error>> {
    let value = value.ok_or("browser is required; expected brave or chrome")?;
    Ok(Browser::from_str(&value)?)
}
