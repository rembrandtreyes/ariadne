use console::style;

pub fn print_header(title: &str) {
    println!("{}", style(title).bold().cyan());
    let separator: String = "\u{2500}".repeat(title.len());
    println!("{}", style(separator).dim());
}

pub fn print_symbol(name: &str, kind: &str, file: &str, line: u32) {
    println!(
        "  {} {} ({}:{})",
        style(kind).dim(),
        style(name).bold(),
        file,
        line
    );
}

pub fn print_success(message: &str) {
    println!("{} {}", style("\u{2713}").green(), message);
}

pub fn print_failure(message: &str) {
    println!("{} {}", style("\u{2717}").red(), message);
}

pub fn print_warning(message: &str) {
    println!("{} {}", style("\u{26A0}").yellow(), message);
}
