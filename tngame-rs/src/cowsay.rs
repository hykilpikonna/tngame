use crate::AsciiArt;

pub fn gen_bubble(text: &str) -> String {
    let mut o = String::with_capacity(text.len() + 100);
    let mut lines = text.lines().map(|line| line.trim());
    let max_width = lines.clone().map(|line| line.len()).max().unwrap();

    o.push_str(".");
    o.push_str("=".repeat(max_width + 2).as_str());
    o.push_str(".\n");
    for line in lines {
        o.push_str("| ");
        o.push_str(line);
        o.push_str(" ".repeat(max_width - line.len()).as_str());
        o.push_str(" |\n");
    }
    o.push_str(".");
    o.push_str("=".repeat(max_width + 2).as_str());
    o.push_str(".\n");
    o
}

pub fn gen_bubble_ascii(text: &str) -> AsciiArt {
    AsciiArt::new(&gen_bubble(text), "cowsay")
}