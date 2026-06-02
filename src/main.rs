use std::io::Write;

fn main() {
    let code = tidemark::cli::run();
    // Flush explicitly: process::exit does not run destructors, so any buffered
    // stdout/stderr must be drained here or a consumer reading our pipe could see
    // truncated output.
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(code);
}
