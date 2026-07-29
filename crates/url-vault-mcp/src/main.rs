use std::io;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(error) = url_vault_mcp::serve(stdin.lock(), stdout.lock()) {
        eprintln!("url-vault-mcp: {error}");
        std::process::exit(1);
    }
}
