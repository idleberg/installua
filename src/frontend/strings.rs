//! String literals: decoding, escape validation, and the two checks that exist
//! because `makensis` will not make them.
//!
//! `full-moon` hands over the raw text between the quotes without looking at
//! it, so `"C:\Program Files"` parses happily here and is a syntax error in
//! real Lua. That gap is why this module exists: the frontend validates escapes
//! itself rather than trusting the parser.

use crate::diag::{Code, Diagnostic, Diagnostics, Span};

/// The vanilla `NSIS_MAX_STRLEN`. A literal at or past this is truncated at
/// runtime with **no diagnostic at all** from `makensis` — verified in Phase 0:
/// 1100 characters compiles clean under `-WX` and measures 1023 at runtime.
pub const MAX_STRING_LENGTH: usize = 1023;

/// What the literal was written as. A long string processes no escapes, and a
/// `raw` argument is NSIS source rather than data, so neither is checked the
/// same way as an ordinary literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiteralKind {
    /// `[[…]]` — no escape processing at all, which is what makes it the
    /// recommended spelling for a Windows path.
    pub long: bool,
    /// A direct argument of `raw`, where `$` is a sigil rather than a dollar
    /// sign and warning about it would be noise.
    pub verbatim: bool,
}

/// Decodes a literal's escapes, diagnosing the ones Lua 5.4 rejects, then runs
/// the two checks NSIS cannot make for itself.
///
/// Decoding never fails: an invalid escape is reported and the backslash kept,
/// so one typo does not cascade into a second diagnostic about the rest of the
/// string.
pub fn decode(raw: &str, kind: LiteralKind, span: Span, diags: &mut Diagnostics) -> String {
    let value = if kind.long {
        raw.to_string()
    } else {
        decode_escapes(raw, span, diags)
    };

    if !kind.verbatim {
        check_dollar(&value, span, diags);
    }
    check_length(&value, span, diags);

    value
}

fn decode_escapes(raw: &str, span: Span, diags: &mut Diagnostics) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        let Some(escape) = chars.next() else {
            // Unterminated trailing backslash. `full-moon` cannot produce this
            // from a well-formed token, so it is defensive rather than reachable.
            out.push('\\');
            break;
        };

        match escape {
            'a' => out.push('\u{7}'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\u{b}'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            // A literal newline after a backslash is a newline.
            '\n' => out.push('\n'),
            '\r' => {
                out.push('\n');
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            // `\z` skips the whitespace that follows it, so a long literal can
            // be wrapped in the source without wrapping in the output.
            'z' => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            'x' => {
                let mut digits = String::new();
                while digits.len() < 2 && chars.peek().is_some_and(char::is_ascii_hexdigit) {
                    digits.push(chars.next().expect("peeked"));
                }
                match u32::from_str_radix(&digits, 16) {
                    Ok(byte) if digits.len() == 2 => out.push(byte_char(byte)),
                    _ => {
                        diags.push(
                            Diagnostic::error(
                                Code::InvalidEscape,
                                span,
                                "`\\x` needs exactly two hexadecimal digits",
                            )
                            .note("Lua 5.4 spells a byte escape `\\xNN`"),
                        );
                        out.push_str("\\x");
                        out.push_str(&digits);
                    }
                }
            }
            'u' => match decode_unicode(&mut chars) {
                Some(c) => out.push(c),
                None => {
                    diags.push(
                        Diagnostic::error(
                            Code::InvalidEscape,
                            span,
                            "`\\u` needs a braced hexadecimal code point",
                        )
                        .note("Lua 5.4 spells it `\\u{1F600}`"),
                    );
                    out.push_str("\\u");
                }
            },
            '0'..='9' => {
                let mut digits = String::from(escape);
                while digits.len() < 3 && chars.peek().is_some_and(char::is_ascii_digit) {
                    digits.push(chars.next().expect("peeked"));
                }
                match digits.parse::<u32>() {
                    Ok(byte) if byte <= 255 => out.push(byte_char(byte)),
                    _ => {
                        diags.push(
                            Diagnostic::error(
                                Code::InvalidEscape,
                                span,
                                format!("`\\{digits}` is out of range for a byte escape"),
                            )
                            .note("a decimal escape is at most `\\255`"),
                        );
                        out.push('\\');
                        out.push_str(&digits);
                    }
                }
            }
            other => {
                diags.push(
                    Diagnostic::error(
                        Code::InvalidEscape,
                        span,
                        format!("`\\{other}` is not a valid escape sequence"),
                    )
                    .note(if other == '/' || other.is_alphanumeric() {
                        "`\\` escapes here, as it does in Lua. \
                         For a Windows path write `\\\\` or a long string `[[…]]`, \
                         or use `/` — it is normalised in path positions"
                            .to_string()
                    } else {
                        format!("write `\\\\{other}` for a backslash followed by `{other}`")
                    }),
                );
                out.push('\\');
                out.push(other);
            }
        }
    }

    out
}

fn decode_unicode(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<char> {
    if chars.peek() != Some(&'{') {
        return None;
    }
    chars.next();

    let mut digits = String::new();
    while chars.peek().is_some_and(char::is_ascii_hexdigit) {
        digits.push(chars.next().expect("peeked"));
    }
    if digits.is_empty() || chars.peek() != Some(&'}') {
        return None;
    }
    chars.next();

    char::from_u32(u32::from_str_radix(&digits, 16).ok()?)
}

/// A byte escape names a byte, and this compiler carries text. Bytes above 127
/// become the matching Latin-1 code point, which round-trips for the ASCII
/// range every real installer uses and is honest about the rest: NSIS cannot
/// spell these at all, so they are folded into the literal either way.
fn byte_char(byte: u32) -> char {
    char::from_u32(byte).unwrap_or('\u{fffd}')
}

/// The muscle-memory trap. A literal is data, so every `$` is escaped to `$$`
/// on output and `detailPrint("costs $5")` is correct — while `detailPrint("in
/// $INSTDIR")` almost never is.
fn check_dollar(value: &str, span: Span, diags: &mut Diagnostics) {
    let bytes: Vec<char> = value.chars().collect();
    for (index, c) in bytes.iter().enumerate() {
        if *c != '$' {
            continue;
        }
        let Some(next) = bytes.get(index + 1) else {
            continue;
        };
        if !(next.is_alphabetic() || *next == '_' || *next == '{' || *next == '(') {
            continue;
        }

        let name: String = bytes[index + 1..]
            .iter()
            .take_while(|c| c.is_alphanumeric() || **c == '_')
            .collect();
        let mut note = String::from(
            "a string literal is data, never a template: every `$` is emitted as `$$`",
        );
        if !name.is_empty() {
            note = format!("did you mean `.. {name} ..`? {note}");
        }

        diags.push(
            Diagnostic::warning(
                Code::DollarInLiteral,
                span,
                format!(
                    "`${}` is emitted as literal text here",
                    first_word(&bytes[index + 1..])
                ),
            )
            .note(note),
        );
        return;
    }
}

fn first_word(rest: &[char]) -> String {
    let word: String = rest
        .iter()
        .take_while(|c| c.is_alphanumeric() || **c == '_')
        .collect();
    if word.is_empty() {
        rest.first().map(|c| c.to_string()).unwrap_or_default()
    } else {
        word
    }
}

/// The 1024-byte check. There is no `makensis` warning to promote here — an
/// over-long string truncate silently — so this is the only place the failure
/// is visible.
fn check_length(value: &str, span: Span, diags: &mut Diagnostics) {
    let length = value.chars().count();
    if length <= MAX_STRING_LENGTH {
        return;
    }
    diags.push(
        Diagnostic::warning(
            Code::OverlongLiteral,
            span,
            format!(
                "this literal is {length} characters, over the {MAX_STRING_LENGTH}-character limit"
            ),
        )
        .note(
            "`makensis` truncates it at runtime and reports nothing; \
             the limit is compiled into `makensis` itself, so raising it means \
             the large-strings build of NSIS",
        ),
    );
}
