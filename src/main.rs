use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("wcag-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.get(1).map(|s| s.as_str()) == Some("check") {
        let patterns: Vec<String> = args[2..].to_vec();
        if patterns.is_empty() {
            eprintln!("Usage: wcag-lsp check <patterns...>");
            std::process::exit(1);
        }
        std::process::exit(wcag_lsp::cli::run_check(&patterns));
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

fn print_help() {
    println!(
        "wcag-lsp v{}
A WCAG accessibility linter for HTML, JSX, TSX, Vue, and Svelte

USAGE:
    wcag-lsp [OPTIONS] [COMMAND]

COMMANDS:
    check <patterns...>    Lint files matching glob patterns
                           Example: wcag-lsp check \"src/**/*.tsx\" \"**/*.html\"

OPTIONS:
    -h, --help             Show this help message
    -v, --version          Print version
        --self-update      Update to latest release",
        env!("CARGO_PKG_VERSION")
    );
}
