//! One-shot, read-only plan transport. No desktop runtime is opened.
fn main() {
    let success = viewer_infrastructure::plan::run_plan_reader(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    );
    if !success {
        std::process::exit(1);
    }
}
