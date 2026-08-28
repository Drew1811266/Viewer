//! One-shot, read-only review transport. No desktop runtime or writable repository is opened.
fn main() {
    let success = viewer_infrastructure::review::run_review_reader(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    );
    if !success {
        std::process::exit(1);
    }
}
