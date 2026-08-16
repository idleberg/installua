//! Running `makensis`, and rewriting what it says through the line map (§15.22).
//!
//! This is the half of the ruling that makes Installua a build tool rather than
//! a program that emits a file, and it is forced rather than chosen: NSIS has
//! `${__LINE__}` for *reading* the compiler's position and nothing for
//! reassigning what it reports, so the map cannot travel inside the artifact
//! and something has to sit between `makensis` and the user.
//!
//! Two message syntaxes exist, verified against NSIS 3.12, and a third class
//! with no line at all:
//!
//! ```text
//! Error in script "w1.nsi" on line 4 -- aborting creation process
//!   6000: unknown variable/constant "UNKNOWNVAR" detected (w3.nsi:3)
//! Error: could not resolve label "nowhere" in unnamed install section (0)
//! ```
//!
//! §14 rules that warnings are failures — a `$`-sigil mistake, a mis-ordered
//! `!define` and an unknown `${FOO}` are all warning 6000 plus a silently wrong
//! installer — so `-WX` is passed here rather than left to the caller, and both
//! syntaxes are on the mapped path.

use std::path::Path;
use std::process::Command;

use crate::map::{LineMap, Origin};

/// One thing `makensis` said, before translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The 1-based line of the **generated** script, when the message names
    /// one. Link-time errors and several warnings name none, and that is a
    /// property of the diagnostic rather than a parsing failure here.
    pub line: Option<usize>,
    pub text: String,
}

/// Every diagnostic in a `makensis` log, in order.
///
/// Lines that are not diagnostics are dropped: `makensis` prints a banner, a
/// size table and a compression summary, and none of it is a message.
pub fn parse(log: &str) -> Vec<Message> {
    let mut out = Vec::new();
    for raw in log.lines() {
        let text = raw.trim();
        // Syntax 1: parse-time. `Error in script "x.nsi" on line 4 -- …`
        if let Some(rest) = text.strip_prefix("Error in script ")
            && let Some((_, tail)) = rest.split_once(" on line ")
        {
            let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
            out.push(Message {
                line: digits.parse().ok(),
                text: text.to_string(),
            });
            continue;
        }

        // Syntax 2: a numbered warning, with the position in a trailing
        // parenthesis — `(w3.nsi:3)`.
        if let Some(position) = numbered_warning(text) {
            out.push(Message {
                line: position,
                text: text.to_string(),
            });
            continue;
        }

        if text.starts_with("Error:") || text.starts_with("Error in ") {
            out.push(Message {
                line: None,
                text: text.to_string(),
            });
        }
    }
    out
}

/// `6000: … (script.nsi:3)` — the warning syntax. Returns the line when the
/// message carries one, and `Some(None)` when it is a warning without a
/// position, which is a real class (`6020` names no line at all).
fn numbered_warning(text: &str) -> Option<Option<usize>> {
    let (code, _) = text.split_once(": ")?;
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let Some(tail) = text.rsplit_once(':') else {
        return Some(None);
    };
    let digits: String = tail
        .1
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() || !tail.1.trim_end().ends_with(')') {
        return Some(None);
    }
    Some(digits.parse().ok())
}

/// One `makensis` message, as the user should see it.
///
/// Three shapes, one per [`Origin`], and the third is the important one: a
/// generated line failing is by definition a compiler bug, so the report says
/// so and names the retained script rather than pointing at code that is not
/// responsible.
pub fn translate(message: &Message, map: &LineMap, source: &str, script: &Path) -> String {
    let script = script.display();
    let origin = message.line.and_then(|line| map.origin(line));
    match (origin, message.line) {
        (Some(Origin::User(span)), _) => {
            format!("{source}:{span}: error[makensis]: {}", message.text)
        }
        (Some(Origin::Raw(span)), _) => format!(
            "{source}:{span}: error[makensis]: {}\n  note: this line is inside a `raw` block, \
             which nothing in this compiler checked (§13)",
            message.text
        ),
        (Some(Origin::Emitted(what)), Some(line)) => format!(
            "error: makensis rejected a line Installua generated ({what}, {script}:{line})\n  \
             note: {}\n  note: this is a compiler bug; the generated script was kept at {script}",
            message.text
        ),
        // No line, or a line past the end of the map: link-time errors name a
        // section rather than a position, and §15.22 rules that inventing one
        // is worse than saying there is none.
        _ => format!(
            "error[makensis]: {}\n  note: this message names no line; the generated script was \
             kept at {script}",
            message.text
        ),
    }
}

/// What running `makensis` produced.
pub struct Assembly {
    pub ok: bool,
    /// The whole log, for the caller that wants to show it.
    pub log: String,
    /// Its diagnostics, already rewritten through the map.
    pub problems: Vec<String>,
}

/// Runs `makensis -WX` over `script` and translates everything it says.
///
/// `-WX` rather than plain: §14's "warnings are failures" is not a test policy,
/// it is the rule that keeps a silently wrong installer from shipping, and the
/// three most common NSIS mistakes are all warning 6000.
pub fn assemble(
    script: &Path,
    map: &LineMap,
    source: &str,
    makensis: &str,
) -> std::io::Result<Assembly> {
    let output = Command::new(makensis)
        .arg("-WX")
        .arg(script)
        // Relative `File` paths in the script resolve against the script, so
        // `makensis` runs there. An empty parent means the script *is* in the
        // current directory, and passing `""` as a working directory is an
        // error rather than a no-op.
        .current_dir(match script.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        })
        .output()?;

    let mut log = String::from_utf8_lossy(&output.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&output.stderr));

    let problems = parse(&log)
        .iter()
        .map(|message| translate(message, map, source, script))
        .collect();

    Ok(Assembly {
        ok: output.status.success(),
        log,
        problems,
    })
}
