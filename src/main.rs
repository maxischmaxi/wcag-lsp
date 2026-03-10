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
        let rest = &args[2..];
        let mut config_path: Option<&str> = None;
        let mut patterns: Vec<String> = Vec::new();
        let mut i = 0;
        while i < rest.len() {
            if (rest[i] == "--config" || rest[i] == "-c") && i + 1 < rest.len() {
                config_path = Some(&rest[i + 1]);
                i += 2;
            } else {
                patterns.push(rest[i].clone());
                i += 1;
            }
        }
        if patterns.is_empty() {
            eprintln!("Usage: wcag-lsp check [--config <path>] <patterns...>");
            std::process::exit(1);
        }
        std::process::exit(wcag_lsp::cli::run_check_with_config(&patterns, config_path));
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
    check [--config <path>] <patterns...>
                           Lint files matching glob patterns
                           Example: wcag-lsp check \"src/**/*.tsx\" \"**/*.html\"
                           Example: wcag-lsp check --config .wcag.toml \"src/**/*.html\"

OPTIONS:
    -h, --help             Show this help message
    -v, --version          Print version
    -c, --config <path>    Path to .wcag.toml or .wcag.json config file
        --self-update      Update to latest release",
        env!("CARGO_PKG_VERSION")
    );
}
