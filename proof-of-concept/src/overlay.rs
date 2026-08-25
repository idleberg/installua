//! The instruction overlay: one data table keyed by Luis name, carrying the
//! NSIS surface spelling and arity. In the real compiler the arity/param kinds
//! half is generated from `makensis -CMDHELP` and only the ergonomics live
//! here; the PoC hand-writes the handful of rows it needs.

use crate::ast::Ty;

pub struct Instruction {
    /// camelCase, as written in Luis source.
    pub luis: &'static str,
    /// NSIS's own spelling, emitted verbatim.
    pub nsis: &'static str,
    pub arity: usize,
}

pub const INSTRUCTIONS: &[Instruction] = &[
    Instruction {
        luis: "detailPrint",
        nsis: "DetailPrint",
        arity: 1,
    },
    Instruction {
        luis: "setOutPath",
        nsis: "SetOutPath",
        arity: 1,
    },
    Instruction {
        luis: "file",
        nsis: "File",
        arity: 1,
    },
    Instruction {
        luis: "delete",
        nsis: "Delete",
        arity: 1,
    },
    Instruction {
        luis: "abort",
        nsis: "Abort",
        arity: 1,
    },
    Instruction {
        luis: "writeUninstaller",
        nsis: "WriteUninstaller",
        arity: 1,
    },
];

pub fn lookup(name: &str) -> Option<&'static Instruction> {
    INSTRUCTIONS.iter().find(|i| i.luis == name)
}

/// Case-insensitive match, so `detailprint` (what everyone arriving from NSIS
/// types) becomes a "did you mean" rather than an unknown global.
pub fn lookup_ignore_case(name: &str) -> Option<&'static Instruction> {
    INSTRUCTIONS
        .iter()
        .find(|i| i.luis.eq_ignore_ascii_case(name))
}

/// Fields accepted inside `installer { ... }`, in emission order.
pub struct InstallerField {
    pub luis: &'static str,
    pub nsis: &'static str,
}

pub const INSTALLER_FIELDS: &[InstallerField] = &[
    InstallerField {
        luis: "name",
        nsis: "Name",
    },
    InstallerField {
        luis: "outFile",
        nsis: "OutFile",
    },
    InstallerField {
        luis: "installDir",
        nsis: "InstallDir",
    },
];

pub fn installer_field(name: &str) -> Option<&'static InstallerField> {
    INSTALLER_FIELDS.iter().find(|f| f.luis == name)
}

/// Options accepted in the table form of `section` and `sectionGroup` — the
/// flags NSIS spells `/o` and `/e`.
pub const SECTION_OPTIONS: &[&str] = &["optional"];
pub const SECTION_GROUP_OPTIONS: &[&str] = &["expanded"];

/// Foreign macros need a declaration, not a mechanism: which header a lowering
/// requires, how many inputs it takes, whether it writes an output variable and
/// whether it needs a one-time init. Same data file as the instruction table
/// above — the compiler emits the `!include` once, and only if something used
/// it.
#[derive(Debug)]
pub struct Header {
    /// As written in `import "…"`.
    pub luis: &'static str,
    pub include: &'static str,
    pub macros: &'static [Macro],
}

#[derive(Debug)]
pub struct Macro {
    pub luis: &'static str,
    /// Spelled without the `${}`; the emitter adds those.
    pub nsis: &'static str,
    pub inputs: usize,
    /// Output position. `None` means the macro is a statement.
    pub output: Option<Ty>,
    /// `${StrCase} $out "s" "U"` puts its destination first; `${WinVerGetMajor}
    /// $out` puts it last. A position, not a convention.
    pub output_first: bool,
    /// Fixed arguments appended after the inputs — how one Luis name selects
    /// one mode of a multi-mode macro.
    pub trailing: &'static [&'static str],
    /// The one-time top-level line the header wants before this macro works.
    /// `StrFunc` hands out its functions this way — the overlay carries more
    /// than ergonomics.
    pub init: Option<&'static str>,
}

pub const HEADERS: &[Header] = &[
    Header {
        luis: "WinVer",
        include: "WinVer.nsh",
        macros: &[
            Macro {
                luis: "getMajor",
                nsis: "WinVerGetMajor",
                inputs: 0,
                output: Some(Ty::Int),
                output_first: false,
                trailing: &[],
                init: None,
            },
            Macro {
                luis: "getMinor",
                nsis: "WinVerGetMinor",
                inputs: 0,
                output: Some(Ty::Int),
                output_first: false,
                trailing: &[],
                init: None,
            },
            Macro {
                luis: "getBuild",
                nsis: "WinVerGetBuild",
                inputs: 0,
                output: Some(Ty::Int),
                output_first: false,
                trailing: &[],
                init: None,
            },
        ],
    },
    Header {
        luis: "x64",
        include: "x64.nsh",
        macros: &[
            Macro {
                luis: "getNativeMachineArchitecture",
                nsis: "GetNativeMachineArchitecture",
                inputs: 0,
                output: Some(Ty::Int),
                output_first: false,
                trailing: &[],
                init: None,
            },
            Macro {
                luis: "disableFSRedirection",
                nsis: "DisableX64FSRedirection",
                inputs: 0,
                output: None,
                output_first: false,
                trailing: &[],
                init: None,
            },
            Macro {
                luis: "enableFSRedirection",
                nsis: "EnableX64FSRedirection",
                inputs: 0,
                output: None,
                output_first: false,
                trailing: &[],
                init: None,
            },
        ],
    },
    Header {
        luis: "StrFunc",
        include: "StrFunc.nsh",
        macros: &[
            Macro {
                luis: "upper",
                nsis: "StrCase",
                inputs: 1,
                output: Some(Ty::Str),
                output_first: true,
                trailing: &["U"],
                init: Some("StrCase"),
            },
            Macro {
                luis: "lower",
                nsis: "StrCase",
                inputs: 1,
                output: Some(Ty::Str),
                output_first: true,
                trailing: &["L"],
                init: Some("StrCase"),
            },
        ],
    },
];

pub fn header(name: &str) -> Option<&'static Header> {
    HEADERS.iter().find(|h| h.luis == name)
}

impl Header {
    pub fn macro_named(&self, name: &str) -> Option<&'static Macro> {
        let macros: &'static [Macro] = self.macros;
        macros.iter().find(|m| m.luis == name)
    }
}

/// Adapted stdlib names: `string.upper` is not a language feature, it is a
/// second spelling of a header macro. Same table, so the `!include` and the
/// init come along for free.
pub struct StdFn {
    pub object: &'static str,
    pub name: &'static str,
    pub header: &'static str,
    pub mac: &'static str,
}

pub const STDLIB: &[StdFn] = &[
    StdFn {
        object: "string",
        name: "upper",
        header: "StrFunc",
        mac: "upper",
    },
    StdFn {
        object: "string",
        name: "lower",
        header: "StrFunc",
        mac: "lower",
    },
];

pub fn stdlib(object: &str, name: &str) -> Option<(&'static Header, &'static Macro)> {
    let entry = STDLIB
        .iter()
        .find(|f| f.object == object && f.name == name)?;
    let header = header(entry.header)?;
    Some((header, header.macro_named(entry.mac)?))
}

/// Whether an object name is a stdlib namespace at all, so `string.reverse`
/// says "no such adaptation" rather than "unbound object".
pub fn is_stdlib_object(object: &str) -> bool {
    STDLIB.iter().any(|f| f.object == object)
}

/// Build-machine commands. These are the ones no amount of constant folding
/// replaces, so they get a visibly different namespace: `pre.`.
#[derive(Debug)]
pub struct Pre {
    pub luis: &'static str,
    pub nsis: &'static str,
    pub inputs: usize,
    /// The symbols the command `!define`s, suffixed onto the prefix it is
    /// handed. `!getdllversion` writes four.
    pub outputs: &'static [&'static str],
    /// How those symbols compose into the value the Luis name stands for.
    pub separator: &'static str,
    pub ty: Ty,
}

pub const PRE: &[Pre] = &[
    Pre {
        luis: "getDllVersion",
        nsis: "!getdllversion",
        inputs: 1,
        outputs: &["1", "2", "3", "4"],
        separator: ".",
        ty: Ty::Str,
    },
    Pre {
        luis: "getTlbVersion",
        nsis: "!gettlbversion",
        inputs: 1,
        outputs: &["1", "2", "3", "4"],
        separator: ".",
        ty: Ty::Str,
    },
];

pub fn pre(name: &str) -> Option<&'static Pre> {
    PRE.iter().find(|p| p.luis == name)
}

/// `MessageBox` is the one command that is a statement, a flag set and a jump
/// table at once, so it gets a bespoke lowering — and these tables are what
/// make its surface checkable rather than a passthrough.
#[derive(Debug)]
pub struct Button {
    /// The handler field in the Luis table.
    pub handler: &'static str,
    /// The NSIS return check, and the spelling `default =` accepts.
    pub id: &'static str,
    /// The label suffix, so a generated label reads as its button.
    pub label: &'static str,
}

#[derive(Debug)]
pub struct ButtonSet {
    pub luis: &'static str,
    pub nsis: &'static str,
    pub buttons: &'static [Button],
}

pub const BUTTON_SETS: &[ButtonSet] = &[
    ButtonSet {
        luis: "OK",
        nsis: "MB_OK",
        buttons: &[Button {
            handler: "onOk",
            id: "IDOK",
            label: "ok",
        }],
    },
    ButtonSet {
        luis: "OKCANCEL",
        nsis: "MB_OKCANCEL",
        buttons: &[
            Button {
                handler: "onOk",
                id: "IDOK",
                label: "ok",
            },
            Button {
                handler: "onCancel",
                id: "IDCANCEL",
                label: "cancel",
            },
        ],
    },
    ButtonSet {
        luis: "YESNO",
        nsis: "MB_YESNO",
        buttons: &[
            Button {
                handler: "onYes",
                id: "IDYES",
                label: "yes",
            },
            Button {
                handler: "onNo",
                id: "IDNO",
                label: "no",
            },
        ],
    },
    ButtonSet {
        luis: "YESNOCANCEL",
        nsis: "MB_YESNOCANCEL",
        buttons: &[
            Button {
                handler: "onYes",
                id: "IDYES",
                label: "yes",
            },
            Button {
                handler: "onNo",
                id: "IDNO",
                label: "no",
            },
            Button {
                handler: "onCancel",
                id: "IDCANCEL",
                label: "cancel",
            },
        ],
    },
    ButtonSet {
        luis: "RETRYCANCEL",
        nsis: "MB_RETRYCANCEL",
        buttons: &[
            Button {
                handler: "onRetry",
                id: "IDRETRY",
                label: "retry",
            },
            Button {
                handler: "onCancel",
                id: "IDCANCEL",
                label: "cancel",
            },
        ],
    },
    ButtonSet {
        luis: "ABORTRETRYIGNORE",
        nsis: "MB_ABORTRETRYIGNORE",
        buttons: &[
            Button {
                handler: "onAbort",
                id: "IDABORT",
                label: "abort",
            },
            Button {
                handler: "onRetry",
                id: "IDRETRY",
                label: "retry",
            },
            Button {
                handler: "onIgnore",
                id: "IDIGNORE",
                label: "ignore",
            },
        ],
    },
];

pub fn button_set(name: &str) -> Option<&'static ButtonSet> {
    BUTTON_SETS.iter().find(|s| s.luis == name)
}

impl ButtonSet {
    pub fn button(&self, handler: &str) -> Option<&'static Button> {
        let buttons: &'static [Button] = self.buttons;
        buttons.iter().find(|b| b.handler == handler)
    }

    /// `default = "NO"` and `default = "IDNO"` both name the same return check.
    pub fn button_by_id(&self, id: &str) -> Option<&'static Button> {
        let buttons: &'static [Button] = self.buttons;
        buttons
            .iter()
            .find(|b| b.id.eq_ignore_ascii_case(id) || b.id[2..].eq_ignore_ascii_case(id))
    }
}

/// The icon axis of `MessageBox`, kept apart from the button axis because they
/// are independent flag groups that happen to share one argument.
#[derive(Debug)]
pub struct Icon {
    pub luis: &'static str,
    pub nsis: &'static str,
}

pub const ICONS: &[Icon] = &[
    Icon {
        luis: "EXCLAMATION",
        nsis: "MB_ICONEXCLAMATION",
    },
    Icon {
        luis: "INFORMATION",
        nsis: "MB_ICONINFORMATION",
    },
    Icon {
        luis: "QUESTION",
        nsis: "MB_ICONQUESTION",
    },
    Icon {
        luis: "STOP",
        nsis: "MB_ICONSTOP",
    },
];

pub fn icon(name: &str) -> Option<&'static Icon> {
    ICONS.iter().find(|i| i.luis == name)
}

pub const MESSAGE_BOX_FIELDS: &[&str] = &["text", "buttons", "icon", "default"];
