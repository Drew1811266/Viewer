//! One-shot stdio MCP server for generation plans. No listener, no daemon.
fn main() {
    viewer_infrastructure::plan::serve(std::io::stdin().lock(), std::io::stdout().lock());
}
