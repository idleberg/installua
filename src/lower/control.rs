//! The controls a `page.custom` lists, and the numbers nsDialogs wants for them
//! (§15.32).
//!
//! `nsDialogs.nsh` spells a control as three defines — a window class, a style
//! word and an extended style word — and `${NSD_CreateLabel}` is nothing but
//! those three glued in front of `nsDialogs::CreateControl`. The table below is
//! that header, read once and written down as numbers, because
//! `PHASE-6-DIALOGS.md` ruling 5 is that **a program that uses a custom page
//! includes nothing**: an emitted `${__NSD_Label_STYLE}` would need
//! `nsDialogs.nsh`, and needing it is the include order this compiler exists to
//! take off the user (§15.7).
//!
//! The style words are folded at compile time rather than emitted as an
//! `|`-chain, so the output carries one number where the header carries six
//! names. The names are here, beside the bits, which is the only place a reader
//! needs them.

/// The three `WS_*` bits every nsDialogs control carries: a child window, drawn
/// now, that does not paint over its siblings.
const DEFAULT_STYLES: u32 = WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS;

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_TABSTOP: u32 = 0x0001_0000;

const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
const WS_EX_WINDOWEDGE: u32 = 0x0000_0100;
const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;

const ES_PASSWORD: u32 = 0x0000_0020;
const ES_AUTOHSCROLL: u32 = 0x0000_0080;
const ES_NUMBER: u32 = 0x0000_2000;

const SS_ETCHEDHORZ: u32 = 0x0000_0010;
const SS_NOTIFY: u32 = 0x0000_0100;
const SS_SUNKEN: u32 = 0x0000_1000;

const BS_AUTOCHECKBOX: u32 = 0x0000_0003;
const BS_GROUPBOX: u32 = 0x0000_0007;
const BS_AUTORADIOBUTTON: u32 = 0x0000_0009;
const BS_VCENTER: u32 = 0x0000_0C00;
const BS_MULTILINE: u32 = 0x0000_2000;

const CBS_DROPDOWNLIST: u32 = 0x0000_0003;
const CBS_AUTOHSCROLL: u32 = 0x0000_0040;
const CBS_HASSTRINGS: u32 = 0x0000_0200;

const LBS_NOTIFY: u32 = 0x0000_0001;
const LBS_HASSTRINGS: u32 = 0x0000_0040;
const LBS_NOINTEGRALHEIGHT: u32 = 0x0000_0100;
const LBS_DISABLENOSCROLL: u32 = 0x0000_1000;

/// The message that appends one string, for the two controls that hold a list.
/// A combo box and a list box are different classes with different message
/// numbers for the same idea, which is why this sits on the control rather than
/// on the `items` option.
const CB_ADDSTRING: u32 = 0x0143;
const LB_ADDSTRING: u32 = 0x0180;

/// One control kind: what a program writes, and what NSIS is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    /// The name the declaration is called by: `label { … }`.
    pub installua: &'static str,
    /// The window class, first argument of `nsDialogs::CreateControl`.
    pub class: &'static str,
    pub style: u32,
    pub exstyle: u32,
    /// What the table's array part means, where the control has one. `None` is
    /// a control with nothing to say — `hLine` draws a rule — and a positional
    /// entry on one of those is an error rather than a string NSIS ignores.
    pub text: Option<&'static str>,
    /// The `ADDSTRING` message, on the two controls that take `items`.
    pub add_item: Option<u32>,
}

impl Control {
    /// Whether this kind accepts `items = { … }`, which is the same question as
    /// whether it has a message for adding one.
    pub fn takes_items(&self) -> bool {
        self.add_item.is_some()
    }
}

/// The kinds, by the name a program writes.
///
/// Thirteen, and the boundary is what a control needs beyond a
/// `CreateControl`: `bitmap` and `link` are the two that do not work as
/// declarations alone — one needs `LoadAndSetImage` and the other needs the
/// click that opens the address — so they land with the field and the event
/// that make them real rather than as controls that draw nothing.
pub const CONTROLS: &[Control] = &[
    Control {
        installua: "label",
        class: "STATIC",
        style: DEFAULT_STYLES | SS_NOTIFY,
        exstyle: WS_EX_TRANSPARENT,
        text: Some("the label's text"),
        add_item: None,
    },
    Control {
        installua: "text",
        class: "EDIT",
        style: DEFAULT_STYLES | WS_TABSTOP | ES_AUTOHSCROLL,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: Some("the text the box starts with"),
        add_item: None,
    },
    Control {
        installua: "password",
        class: "EDIT",
        style: DEFAULT_STYLES | WS_TABSTOP | ES_AUTOHSCROLL | ES_PASSWORD,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: Some("the text the box starts with"),
        add_item: None,
    },
    Control {
        installua: "number",
        class: "EDIT",
        style: DEFAULT_STYLES | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: Some("the number the box starts with"),
        add_item: None,
    },
    Control {
        installua: "button",
        class: "BUTTON",
        style: DEFAULT_STYLES | WS_TABSTOP,
        exstyle: 0,
        text: Some("the button's caption"),
        add_item: None,
    },
    Control {
        installua: "checkbox",
        class: "BUTTON",
        style: DEFAULT_STYLES | WS_TABSTOP | BS_VCENTER | BS_AUTOCHECKBOX | BS_MULTILINE,
        exstyle: 0,
        text: Some("the label beside the box"),
        add_item: None,
    },
    // `BS_AUTORADIOBUTTON` is what makes a row of these exclusive without a
    // line of code: Windows unticks the others in the same group. What it does
    // *not* do is start a group — `WS_GROUP` is nsDialogs'
    // `FirstRadioButton`/`AdditionalRadioButton` pair — so a page with two
    // independent sets of radio buttons is out of scope until there is a
    // spelling for which set a button is in.
    Control {
        installua: "radioButton",
        class: "BUTTON",
        style: DEFAULT_STYLES | WS_TABSTOP | BS_VCENTER | BS_AUTORADIOBUTTON | BS_MULTILINE,
        exstyle: 0,
        text: Some("the label beside the button"),
        add_item: None,
    },
    Control {
        installua: "groupBox",
        class: "BUTTON",
        style: DEFAULT_STYLES | BS_GROUPBOX,
        exstyle: WS_EX_TRANSPARENT,
        text: Some("the heading on the frame"),
        add_item: None,
    },
    Control {
        installua: "hLine",
        class: "STATIC",
        style: DEFAULT_STYLES | SS_ETCHEDHORZ | SS_SUNKEN,
        exstyle: WS_EX_TRANSPARENT,
        text: None,
        add_item: None,
    },
    Control {
        installua: "dropList",
        class: "COMBOBOX",
        style: DEFAULT_STYLES
            | WS_TABSTOP
            | WS_VSCROLL
            | WS_CLIPCHILDREN
            | CBS_AUTOHSCROLL
            | CBS_HASSTRINGS
            | CBS_DROPDOWNLIST,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: None,
        add_item: Some(CB_ADDSTRING),
    },
    Control {
        installua: "listBox",
        class: "LISTBOX",
        style: DEFAULT_STYLES
            | WS_TABSTOP
            | WS_VSCROLL
            | LBS_DISABLENOSCROLL
            | LBS_HASSTRINGS
            | LBS_NOINTEGRALHEIGHT
            | LBS_NOTIFY,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: None,
        add_item: Some(LB_ADDSTRING),
    },
    // Both are plain edit boxes: nsDialogs' `FileRequest` and `DirRequest` are
    // the same class as `text` and differ only in what the browse button beside
    // them does — and that button is `${NSD_CreateBrowseButton}`, a control of
    // its own that this surface does not draw for you.
    Control {
        installua: "fileRequest",
        class: "EDIT",
        style: DEFAULT_STYLES | WS_TABSTOP | ES_AUTOHSCROLL,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: Some("the path the box starts with"),
        add_item: None,
    },
    Control {
        installua: "dirRequest",
        class: "EDIT",
        style: DEFAULT_STYLES | WS_TABSTOP | ES_AUTOHSCROLL,
        exstyle: WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
        text: Some("the path the box starts with"),
        add_item: None,
    },
];

/// The kind this name declares, or `None` for a name that declares no control.
pub fn control(name: &str) -> Option<&'static Control> {
    CONTROLS.iter().find(|control| control.installua == name)
}

/// The kinds, for the error that has to list them.
pub fn names() -> Vec<&'static str> {
    CONTROLS.iter().map(|control| control.installua).collect()
}
