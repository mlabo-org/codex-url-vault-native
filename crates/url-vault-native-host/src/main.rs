use std::io;

use url_vault_core::Vault;
use url_vault_native_host::{brave_manifest, serve};

fn main() {
    if let Err(error) = run() {
        eprintln!("url-vault-native-host: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args();
    let _program = arguments.next();
    if arguments.next().as_deref() == Some("--print-brave-manifest") {
        let host_path = arguments
            .next()
            .ok_or("--print-brave-manifest requires an absolute host path")?;
        if !std::path::Path::new(&host_path).is_absolute() {
            return Err("native host path must be absolute".into());
        }
        serde_json::to_writer_pretty(io::stdout().lock(), &brave_manifest(&host_path))?;
        println!();
        return Ok(());
    }

    let vault = Vault::from_env()?;
    vault.init()?;
    serve(io::stdin().lock(), io::stdout().lock(), &vault)?;
    Ok(())
}
