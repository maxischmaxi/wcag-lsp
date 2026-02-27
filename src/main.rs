use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("wcag-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|a| a == "--self-update") {
        if let Err(e) = wcag_lsp::updater::self_update().await {
            eprintln!("Update failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(wcag_lsp::server::WcagLspServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
