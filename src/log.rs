//! The CLI's own output: a line, under one of clark's coloured symbols.
//!
//! ```text
//! ◆  wrote installua.toml
//! ●  now run `installua stubs`
//! ▲  installua.toml exists, left alone
//! ■  cannot read main.lua
//! ```
//!
//! The symbols and their colours are clark's `Theme::clack()`, spelled out here
//! as the four SGR codes they render to; the Guide's bar is not, because these
//! lines are not steps in a prompt session. `log` is the level with no symbol,
//! and prints the text alone. Nothing but the symbol is coloured — a path or a
//! flag inside a message is marked the way it is in the source's own prose,
//! with backticks, which survive being piped into a file.
//!
//! Not `clark::log`, which draws that bar and writes to stdout: `emit --stdout`
//! puts the compiled script on stdout and `coverage` puts a golden file there,
//! so a line of ours on that stream would end up inside the artifact.
//! Everything here goes to stderr — all of it, so two levels in a row keep
//! their order. A failed write is dropped: these are called for their side
//! effect by a program with nothing useful to do about a broken pipe.

use std::io::{IsTerminal, Write};

/// A line with no symbol of its own.
pub fn log(text: impl AsRef<str>) {
    emit("", 0, text.as_ref());
}

/// A line under a blue `●`.
pub fn info(text: impl AsRef<str>) {
    emit("●", 34, text.as_ref());
}

/// A line under a green `◆`.
pub fn success(text: impl AsRef<str>) {
    emit("◆", 32, text.as_ref());
}

/// A line under a yellow `▲`.
pub fn warn(text: impl AsRef<str>) {
    emit("▲", 33, text.as_ref());
}

/// A line under a red `■`.
pub fn error(text: impl AsRef<str>) {
    emit("■", 31, text.as_ref());
}

/// The symbol, two spaces, and the text — or the text alone when there is no
/// symbol. Rows after the first are indented to sit under it.
fn render(symbol: &str, colour: u8, text: &str, colours: bool) -> String {
    if symbol.is_empty() {
        return format!("{text}\n");
    }
    let indented = text.replace('\n', "\n   ");
    format!("{}  {indented}\n", paint(symbol, colour, colours))
}

fn paint(text: &str, colour: u8, colours: bool) -> String {
    match colours {
        true => format!("\u{1b}[{colour}m{text}\u{1b}[0m"),
        false => text.to_string(),
    }
}

fn emit(symbol: &str, colour: u8, text: &str) {
    // Asked of stderr, which is where this writes: a run whose stdout is piped
    // into a file still has a terminal reading the messages.
    let colours = std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal();
    let _ = std::io::stderr().write_all(render(symbol, colour, text, colours).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_rows_line_up_under_the_first() {
        assert_eq!(render("■", 31, "one\ntwo", false), "■  one\n   two\n");
    }

    #[test]
    fn the_colour_wraps_the_symbol_and_not_the_text() {
        assert_eq!(
            render("▲", 33, "careful", true),
            "\u{1b}[33m▲\u{1b}[0m  careful\n"
        );
    }

    #[test]
    fn the_bare_level_is_the_text_alone() {
        assert_eq!(render("", 0, "plain", true), "plain\n");
    }
}
