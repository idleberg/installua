//! The `-CMDHELP` generator: syntax lines in, parameter skeletons out.
//!
//! The parameter model is joined from two halves, and this is the mechanical
//! one. `makensis -CMDHELP` prints one syntax line per command and is the only
//! machine-readable description NSIS ships, so arity, optionality, flags, enum
//! members and — crucially — which positions are *variables* rather than values
//! are read from it rather than transcribed by hand.
//!
//! What is **not** read from it: the Installua name, the census class, the
//! types and which positions are paths. Those are judgements, they live in
//! [`super::overlay`], and the join test asserts the two halves cover each
//! other exactly.
//!
//! The output of this module is rendered to Rust source and checked in
//! ([`generate`]), so a new NSIS version is a reviewable diff rather than a
//! silent change in behaviour. Two tests guard it: one regenerates from the
//! checked-in snapshot and diffs the generated file, one regenerates the
//! snapshot from the local `makensis` and diffs that.

use super::{Dir, Note, Rep};

/// One `-CMDHELP` line, parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub nsis: String,
    pub params: Vec<ParsedParam>,
    pub options: Vec<ParsedOpt>,
    pub note: Note,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedParam {
    /// The `-CMDHELP` spelling, kept so an enum annotation on a later line can
    /// find its parameter and so the generated file is readable.
    pub name: String,
    pub dir: Dir,
    /// A `$(user_var: …)` position: an NSIS *variable* is demanded, not a
    /// value. A `dir: In` on a variable — `fileRead(open())` has no lowering,
    /// and this is the field that says so.
    pub var: bool,
    pub req: bool,
    pub rep: Rep,
    /// `mode=SET|CUR|END`, read off the indented continuation line.
    pub members: Vec<String>,
    /// The member list ends in a **placeholder** — `…|Win10|{GUID}` — so the
    /// keywords are the ones worth completing and not the ones worth enforcing.
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedOpt {
    pub nsis: String,
    /// `/TIMEOUT=X` rather than `/BRANDING`.
    pub value: bool,
    /// Emit position: the number of parameters that precede it.
    pub after: usize,
}

/// NSIS prints prose in the syntax position for retired commands. There are
/// five in 3.12 and they are recognised by marker rather than by name, so a
/// sixth in 3.13 is classified rather than mis-parsed.
const PROSE: &[&str] = &[
    "deprecated",
    "obsolete",
    "no longer supported",
    "doesn't currently work",
];

/// Parses a checked-in snapshot. Blank lines and `#` comments are skipped, so
/// the snapshot can record which `makensis` printed it.
pub fn parse(snapshot: &str) -> Vec<Parsed> {
    let mut commands: Vec<Parsed> = Vec::new();

    for line in snapshot.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        // A continuation line is indented; a command starts at column 0. That
        // is the only structure `-CMDHELP` has, and it is stable across every
        // version this has been run against.
        if line.starts_with(char::is_whitespace) {
            if let Some(last) = commands.last_mut() {
                continuation(last, line.trim());
            }
            continue;
        }

        let (name, rest) = match line.split_once(char::is_whitespace) {
            Some((name, rest)) => (name, rest.trim()),
            None => (line, ""),
        };
        commands.push(command(name, rest));
    }

    commands
}

fn command(name: &str, syntax: &str) -> Parsed {
    let lower = syntax.to_ascii_lowercase();
    if PROSE.iter().any(|marker| lower.contains(marker)) {
        return Parsed {
            nsis: name.to_string(),
            params: Vec::new(),
            options: Vec::new(),
            note: Note::Prose,
        };
    }

    if name.starts_with('!') {
        return Parsed {
            nsis: name.to_string(),
            params: Vec::new(),
            options: Vec::new(),
            note: Note::Directive,
        };
    }

    let mut command = Parsed {
        nsis: name.to_string(),
        params: Vec::new(),
        options: Vec::new(),
        note: Note::Plain,
    };
    parse_into(&mut command, syntax, true);
    command
}

/// Walks one syntax fragment, appending to `command`.
///
/// `required` is threaded rather than tracked on a stack because NSIS nests
/// optionals to express *trailing* optionality — `[a [b [c]]]` means "a, then
/// maybe b, then maybe c" — so a nested optional is flat as far as the
/// parameter list is concerned.
fn parse_into(command: &mut Parsed, syntax: &str, required: bool) {
    parse_tokens(command, tokenize(syntax), required);
}

fn parse_tokens(command: &mut Parsed, tokens: Vec<Token>, required: bool) {
    for token in tokens {
        match token {
            Token::Alternation => {
                command.note = Note::Alternation;
                // What follows is the second spelling of the same command, and
                // it goes to the overlay as a `conflicts` set rather than to
                // the parameter list.
                return;
            }
            Token::Repeat => {
                if let Some(last) = command.params.last_mut() {
                    last.rep = Rep::Many;
                }
            }
            Token::Group { text, optional } => {
                // `[/x filespec [...]]` and `[/SD return]`: a bracketed group
                // whose first token is a flag and whose rest are plain words is
                // a flag *with an argument*, not a flag followed by a
                // parameter. The argument goes to the `Opt` — one `exclude =
                // {…}` expands to N `/x` pairs — so the words after the flag
                // must not become positional parameters, which would shift
                // every later position by one.
                if let Some(flag) = flag_with_argument(&text) {
                    command.options.push(ParsedOpt {
                        nsis: flag,
                        value: true,
                        after: command.params.len(),
                    });
                    continue;
                }
                let mut inner = tokenize(&text);
                if optional {
                    merge_prose(&mut inner);
                }
                parse_tokens(command, inner, required && !optional);
            }
            Token::Word(word) => word_param(command, &word, required),
        }
    }
}

/// `"/x filespec [...]"` ⇒ `Some("/x")`. `None` for anything else, including
/// `(/windows | (fg bg))`, where the words after the flag are the *other* side
/// of an alternation rather than its argument.
fn flag_with_argument(text: &str) -> Option<String> {
    let mut words = text.split_whitespace();
    let first = words.next()?;
    if !first.starts_with('/') || first.contains('=') {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    if rest.is_empty() || rest.iter().any(|word| word.contains('|')) {
        return None;
    }
    Some(first.to_string())
}

/// `[back button text]` is **one** optional parameter whose name has spaces in
/// it, and `[return_check label_to_goto_if_equal …]` is two.
///
/// `-CMDHELP` separates positions with spaces and writes multi-word names with
/// spaces too, so the notation is ambiguous on its face. Three things resolve
/// it, and a group needs all three:
///
/// - **nothing but words.** `CreateFont … [height weight /ITALIC /UNDERLINE
///   /STRIKE]` is five positions, and a group that goes on to list flags is a
///   tail of positions rather than a name.
/// - **no nested optional.** `[icon index [showmode …]]` and `[return_check
///   label_to_goto_if_equal [return_check2 label2]]` both open a further
///   position, which is what a name does not do. `CreateShortcut`'s two words
///   *are* one argument, and the overlay says so with [`Kind::Fused`] — the
///   snapshot keeps printing what `makensis` prints.
/// - **no underscore.** Every multi-word parameter NSIS names — `top_color`,
///   `accept_text`, `pre_function`, `label_to_goto_if_equal` — joins its words
///   with one, so words that use none are English rather than names.
///
/// What is left is nine groups in 3.12, all of them a caption: `[back button
/// text]`, `[space required text]`, `[text without ignore]`.
///
/// Only inside a bracket, because a *required* run of words really is several
/// positions: `PEAddResource … file restype resname` is three.
///
/// [`Kind::Fused`]: super::Kind::Fused
fn merge_prose(tokens: &mut Vec<Token>) {
    if tokens.len() < 2 {
        return;
    }
    let words: Option<Vec<&str>> = tokens
        .iter()
        .map(|token| match token {
            // `/OVERWRITE|/REPLACE` and `state(1|0)` are a choice, not a phrase.
            Token::Word(word)
                if !word.contains('_') && !word.starts_with('/') && !word.contains('|') =>
            {
                Some(word.as_str())
            }
            _ => None,
        })
        .collect();
    let Some(words) = words else {
        return;
    };

    *tokens = vec![Token::Word(words.join("_"))];
}

fn word_param(command: &mut Parsed, word: &str, required: bool) {
    if word.starts_with('/') {
        // `/TIMEOUT=X` takes a value; `/RESIZETOFIT[WIDTH|HEIGHT]` does not —
        // the bracket is a suffix that spells a *different* flag, so it is
        // trimmed off the name rather than read as an argument.
        let name: String = word
            .split(['=', '[', '('])
            .next()
            .unwrap_or(word)
            .to_string();
        command.options.push(ParsedOpt {
            nsis: name,
            value: word.contains('='),
            after: command.params.len(),
        });
        return;
    }

    // `$(user_var: handle input)`, and the four other spellings 3.12 uses.
    if let Some(inner) = word
        .strip_prefix("$(")
        .and_then(|word| word.strip_suffix(')'))
    {
        let (_, description) = inner.split_once(':').unwrap_or((inner, ""));
        let description = description.trim();
        // **Out unless it says otherwise.** `-CMDHELP` spells an output five
        // ways — `output`, `result`, `return value`, `imagehandle`, and a bare
        // `$(user_var)` — and an input exactly two: `input` and `in/out`. The
        // rule keyed on the outputs missed three commands; keyed on the inputs
        // it misses none, and a *new* spelling defaults to the direction that
        // costs a register rather than the one that silently drops a result.
        let dir = if description.contains("input") || description.contains("in/out") {
            Dir::In
        } else {
            Dir::Out
        };
        push(
            command,
            ParsedParam {
                name: if description.is_empty() {
                    "var".to_string()
                } else {
                    description.trim_end_matches(')').replace(' ', "_")
                },
                dir,
                var: true,
                req: required,
                rep: Rep::One,
                members: Vec::new(),
                open: false,
            },
        );
        return;
    }

    // `file...` is `file` again, and `state(1|0)` is `state` with two members:
    // both are one parameter written as two things.
    let (word, repeated) = match word.strip_suffix("...") {
        Some(stem) => (stem, true),
        None => (word, false),
    };
    // `attribute[|attribute[...]]` is the parameter `attribute`, repeated
    // inside one argument. The name has to be the plain one or the
    // `attribute=(NORMAL|…)` line below it never finds its parameter.
    let word = match word.split_once('[') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => word,
    };
    let (word, parenthesised) = match word.split_once('(') {
        Some((stem, rest)) if !stem.is_empty() => (stem, rest.trim_end_matches(')')),
        _ => (word, ""),
    };

    // `on|off` written inline, without a continuation line to name it.
    let source = if parenthesised.is_empty() {
        word
    } else {
        parenthesised
    };
    let Alternation {
        members,
        open,
        named,
    } = split_members(source);

    let name = if named.is_empty() || !parenthesised.is_empty() {
        word.to_string()
    } else {
        named.join("_or_")
    };

    push(
        command,
        ParsedParam {
            name,
            dir: Dir::In,
            var: false,
            req: required,
            rep: if repeated { Rep::Many } else { Rep::One },
            members,
            open,
        },
    );
}

/// Appends a parameter, collapsing `InstTypeIdx [InstTypeIdx [...]]` — one
/// repeated parameter that `-CMDHELP` writes twice — into a single `Rep::Many`.
fn push(command: &mut Parsed, param: ParsedParam) {
    if let Some(last) = command.params.last_mut()
        && last.name == param.name
        && last.var == param.var
    {
        last.rep = Rep::Many;
        return;
    }
    if param.rep == Rep::Many
        && let Some(last) = command.params.last_mut()
        && last.name == param.name
    {
        last.rep = Rep::Many;
        return;
    }
    command.params.push(param);
}

/// The alternatives `-CMDHELP` writes as bare words that are not keywords.
///
/// A metavariable normally carries a marker — `{GUID}`, `$(user_var: …)`, a
/// `[…]` for optional — and [`split_members`] reads the marker rather than the
/// word. These three have none: they are spelled exactly like the keywords
/// beside them, and only `makensis` can tell them apart. It does, by rejecting
/// the literal word:
///
/// ```text
/// PERemoveResource "#5" "#105" reslang   Usage: PERemoveResource …
/// AddBrandingImage top height            Invalid number!
/// AddBrandingImage top width             Invalid number!
/// ```
///
/// Written out rather than inferred, because inference does not work here. The
/// rule that catches all three — *a lowercase alternative beside one carrying a
/// capital* — also fires on `PageEx custom|uninstConfirm|…` and on
/// `ManifestSupportedOS none|all|WinVista|…`, where the lowercase words are
/// real keywords. It would strip seven of them and silently open two closed
/// sets, which trades this file's defect for a worse one: a check that no
/// longer runs and says nothing about it.
///
/// Kept short on purpose. A word belongs here once `makensis` has been run
/// against it and rejected it, and not because a reader thought it looked like a
/// placeholder — `SendMessage`'s `wparam|STR:wParam` is the same shape and is
/// absent, because nothing lowers that row and an unverified entry is a guess
/// this table cannot check.
const METAVARIABLES: &[&str] = &["reslang", "height", "width"];

/// What one alternation yields: the members worth checking against, whether the
/// set is open, and the words the parameter is *named* after.
///
/// `named` is not `members` because a [`METAVARIABLES`] entry is dropped from
/// one and kept in the other. `reslang|ALL` has one member and is the parameter
/// `reslang`, and `(height|width)` has none and is still the parameter
/// `height_or_width`: a metavariable is the best name a position has, which is
/// what makes it a metavariable and not a keyword. A `{GUID}` is in neither —
/// braces are not a name, and the row they appear on already has one.
struct Alternation {
    members: Vec<String>,
    open: bool,
    named: Vec<String>,
}

/// Splits an enum's members.
///
/// Two separators, and which one is in use is decided by the text rather than
/// by the command: `mode=SET|CUR|END` separates with `|`, while
/// `OP=(+ - * / % | & ^ ~ ! || && << >> >>>)` separates with spaces *because
/// `|` is itself a member*. Splitting the second on `|` loses the two operators
/// a user is most likely to get wrong.
fn split_members(text: &str) -> Alternation {
    // The braces around `flag={smooth|colored}` wrap the whole list and mean
    // nothing; the ones around `{GUID}` wrap **one** alternative and mean it is
    // a placeholder. Trimming the outer pair first is what keeps the two apart,
    // since no annotation in 3.12 is a lone placeholder.
    let empty = || Alternation {
        members: Vec::new(),
        open: false,
        named: Vec::new(),
    };
    let text = text.trim();
    let text = match text.chars().next() {
        Some('(') => text.strip_prefix('(').and_then(|t| t.strip_suffix(')')),
        Some('{') => text.strip_prefix('{').and_then(|t| t.strip_suffix('}')),
        _ => None,
    }
    .unwrap_or(text);
    if text.is_empty() {
        return empty();
    }

    // `mode=modeflag[|modeflag[...]]]` describes recursion, not a member list:
    // the real members are on the next line, which NSIS itself truncates. An
    // unparseable annotation is no annotation — the overlay says what
    // `MessageBox`'s flags are, and `messageBox` already ruled on them.
    if text.contains("...") {
        return empty();
    }

    // A bracket surviving the trim above means the annotation was a *grammar*
    // and not a list. `hotkey=(ALT|CONTROL|EXT|SHIFT)|(F1-F24|A-Z)` says pick
    // modifiers, join them to one key with `|` — so no whole value it describes
    // is a member of it. Flattened, it recorded six: four legal alone, and two
    // that are ranges written as words and cannot compile at all. `makensis`
    // took `"CONTROL|SHIFT|Z"` while the alias generated from those six marked
    // it wrong, which is the direction that matters — a stub narrower than the
    // compiler reports errors in correct programs.
    //
    // Open rather than [`empty`], and the difference is a claim about NSIS
    // rather than about this parser: the values are not unknown here, they are
    // uncountable, so nothing downstream should ever check against a list.
    if text.contains('(') || text.contains(')') {
        return Alternation {
            members: Vec::new(),
            open: true,
            named: Vec::new(),
        };
    }

    let text = expand_families(text);
    let raw: Vec<String> = if text.contains(char::is_whitespace) {
        text.split_whitespace().map(str::to_string).collect()
    } else if text.contains('|') {
        text.split('|').map(str::to_string).collect()
    } else {
        return empty();
    };

    // `{GUID}` is not a keyword. NSIS wraps a **placeholder** in braces where a
    // keyword would go, and `makensis` rejects the literal word `GUID`, so
    // recording it as a member offers a completion that cannot compile and
    // — worse — makes the check reject the real GUIDs it stands for.
    //
    // [`METAVARIABLES`] is the same thing said without the braces, which is why
    // it is a list and not a rule.
    let mut open = false;
    let mut members = Vec::new();
    let mut named = Vec::new();
    for word in raw {
        let word = word.trim();
        let braced = word.starts_with('{') && word.ends_with('}');
        let word = word.trim_matches(|c| "()[]".contains(c));
        if word.is_empty() {
            continue;
        }
        let bare = METAVARIABLES.contains(&word);
        open |= braced || bare;
        if !braced && !bare {
            members.push(word.to_string());
        }
        // A braced placeholder is dropped from the name as well; see
        // [`Alternation`] for why a bare one is not.
        if !braced {
            named.push(word.to_string());
        }
    }
    Alternation {
        members,
        open,
        named,
    }
}

/// `HKLM[32|64]` is three registry roots written as one member, and `SHCTX`
/// beside it is one written as one. Expanding before the split is what keeps
/// the `Enum` check from rejecting `HKLM64`, which is a root a user really can
/// write — and doing it after produces the members `HKLM[32` and `64]`, which
/// is how this was wrong the first time.
fn expand_families(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;

    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else {
            break;
        };
        let close = open + close;
        let stem_start = rest[..open]
            .rfind(|c: char| c == '|' || c.is_whitespace())
            .map(|index| index + 1)
            .unwrap_or(0);
        let stem = &rest[stem_start..open];

        out.push_str(&rest[..open]);
        for suffix in rest[open + 1..close].split('|') {
            out.push('|');
            out.push_str(stem);
            out.push_str(suffix);
        }
        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    out
}

/// An indented line: either `name=members`, which annotates a parameter, or the
/// wrapped tail of the syntax above it.
fn continuation(command: &mut Parsed, line: &str) {
    let Some((key, values)) = line.split_once('=') else {
        // `File`'s second line. It is the far side of an alternation, which the
        // first line already recorded.
        command.note = Note::Alternation;
        return;
    };

    if key.contains(char::is_whitespace) || key.starts_with('/') {
        command.note = Note::Alternation;
        return;
    }

    let Alternation { members, open, .. } = split_members(values);

    // `root_key` annotates the parameter `-CMDHELP` spells `rootkey`, and
    // `OP` annotates `OP`. Both match once the punctuation and case are gone.
    let wanted = normalise(key);
    for param in &mut command.params {
        if normalise(&param.name) == wanted {
            param.members = members;
            param.open = open;
            return;
        }
    }
}

fn normalise(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

enum Token {
    Word(String),
    Group { text: String, optional: bool },
    Alternation,
    Repeat,
}

/// Splits a syntax fragment at top level, keeping bracketed and parenthesised
/// groups whole. `$(…)` is a word rather than a group, because it names one
/// parameter.
fn tokenize(syntax: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = syntax.chars().peekable();
    let mut word = String::new();

    while let Some(c) = chars.next() {
        match c {
            '[' | '(' => {
                let close = if c == '[' { ']' } else { ')' };
                let mut text = String::new();
                let mut inner = 1usize;
                for c in chars.by_ref() {
                    if c == '[' || c == '(' {
                        inner += 1;
                    } else if c == close || c == ']' || c == ')' {
                        inner -= 1;
                        if inner == 0 {
                            break;
                        }
                    }
                    text.push(c);
                }

                // A bracket that *follows* a word belongs to it —
                // `$(user_var: text output)`, `state(1|0)`,
                // `/RESIZETOFIT[WIDTH|HEIGHT]` — and one that stands alone is a
                // group. Splitting the first kind is what turned `state(1|0)`
                // into an alternation between two commands.
                if !word.is_empty() {
                    word.push(c);
                    word.push_str(&text);
                    word.push(close);
                    continue;
                }

                // `[text (can contain $0)]`: a parenthesised group that follows
                // a *word* annotates it, and prose in a parameter list is not a
                // parameter list. A group that follows a group is a second
                // choice — `(top|left|…) (height|width)` — and one that opens
                // the fragment is the first, so neither is commentary.
                if c == '(' && matches!(tokens.last(), Some(Token::Word(_))) {
                    continue;
                }

                if text.trim() == "..." || text.trim() == "…" {
                    tokens.push(Token::Repeat);
                } else {
                    tokens.push(Token::Group {
                        text,
                        optional: c == '[',
                    });
                }
            }
            '|' if word.is_empty() => tokens.push(Token::Alternation),
            c if c.is_whitespace() => flush(&mut tokens, &mut word),
            c => word.push(c),
        }
    }
    flush(&mut tokens, &mut word);
    tokens
}

fn flush(tokens: &mut Vec<Token>, word: &mut String) {
    if word.is_empty() {
        return;
    }
    let taken = std::mem::take(word);
    if taken == "..." || taken == "…" {
        tokens.push(Token::Repeat);
    } else if taken == "|" {
        tokens.push(Token::Alternation);
    } else {
        tokens.push(Token::Word(taken));
    }
}

/// Renders the parsed snapshot as the Rust source checked in at
/// `src/table/generated.rs`.
///
/// Generated *code* rather than a parse at startup, so that the per-version
/// diff is reviewable and the table is `&'static` with no initialisation order
/// to get wrong.
pub fn generate(snapshot: &str) -> String {
    let commands = parse(snapshot);
    let version = snapshot
        .lines()
        .find_map(|line| line.strip_prefix("# makensis "))
        .unwrap_or("unknown")
        .trim();

    let mut out = String::new();
    out.push_str(&format!(
        "//! Generated from `makensis -CMDHELP` ({version}). Do not edit.\n\
         //! \n\
         //! \n\
         //! ```text\n\
         //! cargo run -q -- generate table tables/cmdhelp-3.12.txt > /tmp/generated.rs\n\
         //! mv /tmp/generated.rs src/table/generated.rs\n\
         //! ```\n\
         //! \n\
         //! `cargo test` fails if this file and the snapshot ever drift, and\n\
         //! again if the snapshot and the local `makensis` do. The\n\
         //! hand-written half of every row is in `super::overlay`.\n\
         \n\
         use super::{{Dir, Note, Opt, Rep, Shape}};\n\
         \n\
         /// One entry per `-CMDHELP` line, in the order `makensis` prints them.\n\
         pub struct Skeleton {{\n\
         \x20   pub nsis: &'static str,\n\
         \x20   pub params: &'static [Shape],\n\
         \x20   pub options: &'static [Opt],\n\
         \x20   pub note: Note,\n\
         }}\n\
         \n\
         pub const VERSION: &str = \"{version}\";\n\
         \n\
         pub const SKELETONS: &[Skeleton] = &[\n"
    ));

    for command in &commands {
        out.push_str(&format!(
            "    Skeleton {{\n        nsis: {:?},\n",
            command.nsis
        ));
        out.push_str("        params: &[");
        for param in &command.params {
            out.push_str(&format!(
                "\n            Shape {{ name: {:?}, dir: Dir::{:?}, var: {}, req: {}, rep: Rep::{:?}, members: &[{}], open: {} }},",
                param.name,
                param.dir,
                param.var,
                param.req,
                param.rep,
                param
                    .members
                    .iter()
                    .map(|member| format!("{member:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                param.open,
            ));
        }
        out.push_str(if command.params.is_empty() {
            "],\n"
        } else {
            "\n        ],\n"
        });

        out.push_str("        options: &[");
        for opt in &command.options {
            out.push_str(&format!(
                "\n            Opt {{ nsis: {:?}, value: {}, after: {} }},",
                opt.nsis, opt.value, opt.after
            ));
        }
        out.push_str(if command.options.is_empty() {
            "],\n"
        } else {
            "\n        ],\n"
        });

        out.push_str(&format!(
            "        note: Note::{:?},\n    }},\n",
            command.note
        ));
    }

    out.push_str("];\n");
    out
}
