fn render_terminal(title: &str) -> String {
    format!("Well: {title}")
}

fn retry_delay(attempt: u32) -> u32 {
    // BUG: the retry delay should grow with each attempt.
    100
}

fn main() {
    println!("{}", render_terminal("ready"));
}
