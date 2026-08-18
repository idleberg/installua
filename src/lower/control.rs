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

const SS_BITMAP: u32 = 0x0000_000E;

/// The message that appends one string, for the two controls that hold a list.
/// A combo box and a list box are different classes with different message
/// numbers for the same idea, which is why this sits on the control rather than
/// on the `items` option.
const CB_ADDSTRING: u32 = 0x0143;
const LB_ADDSTRING: u32 = 0x0180;

/// The messages a field is, where the field is one.
///
/// There is no `WM_GETTEXT` here, and its absence is the one asymmetry in the
/// field surface: NSIS's `SendMessage` has no way to be handed a buffer, so a
/// control's text is *read* by `System::Call user32::GetWindowText` — which is
/// what `nsDialogs.nsh` does too, and which needs no header because the plugin
/// is `System` and the length is makensis' own `${NSIS_MAX_STRLEN}`.
pub const WM_SETTEXT: u32 = 0x000C;
pub const WM_SETFONT: u32 = 0x0030;
pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;

/// `ShowWindow`'s two states. Named rather than `!define`d, on ruling 5.
pub const SW_HIDE: u32 = 0;
pub const SW_SHOW: u32 = 5;

/// `LoadAndSetImage`'s two constants: what kind of image, and where it comes
/// from. A `.bmp` beside the installer at run time, loaded by name.
pub const IMAGE_BITMAP: u32 = 0;
pub const LR_LOADFROMFILE: u32 = 0x0010;

/// The weight `CreateFont` wants for `bold = true`, and the one it wants for
/// `bold = false`. Windows' scale runs 0–1000 and names nine points on it; these
/// are the two anything reads back.
pub const FW_NORMAL: u32 = 400;
pub const FW_BOLD: u32 = 700;

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

    /// Whether the kind holds a tick.
    ///
    /// It is the `BS_AUTOCHECKBOX`/`BS_AUTORADIOBUTTON` style that makes one so,
    /// and this names the kinds rather than masking for it: the low bits of a
    /// `BS_` style are a small enum and not flags — `BS_GROUPBOX` is 7 and
    /// `BS_AUTOCHECKBOX` is 3 — so `style & BS_AUTOCHECKBOX` is true of a group
    /// box, and a mask here would be a bug that reads like an optimisation.
    pub fn checkable(&self) -> bool {
        matches!(self.installua, "checkbox" | "radioButton")
    }

    /// Whether the kind draws a picture rather than text, and so is the one
    /// that answers to `image`.
    pub fn imageable(&self) -> bool {
        matches!(self.installua, "bitmap")
    }
}

/// The kinds, by the name a program writes.
///
/// Fourteen, and the boundary is what a control needs beyond a
/// `CreateControl`: `link` is the one that does not work as a declaration alone,
/// since what it is *for* is the click that opens the address, so it lands with
/// the event rather than as a control that draws nothing. `bitmap` arrived with
/// `image`, which is the field that gives it something to draw.
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
    // A `STATIC` that holds a picture instead of a caption, which is why it has
    // no text: `CreateControl`'s last argument is ignored by a `SS_BITMAP`
    // window, and what it draws arrives afterwards through `image`.
    Control {
        installua: "bitmap",
        class: "STATIC",
        style: DEFAULT_STYLES | SS_BITMAP | SS_NOTIFY,
        exstyle: 0,
        text: None,
        add_item: None,
    },
];

/// A field of a control handle: `serial.value`, `agree.checked = true`.
///
/// Seven, against a section's seven, and the shape is different in one way that
/// matters: a section's flags are a word that has to be read, edited and written
/// back, and a control's field is one instruction each. Nothing here is a
/// read-modify-write, so nothing here needs a temporary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlField {
    /// The window's text. `System::Call user32::GetWindowText` to read,
    /// `SendMessage WM_SETTEXT` to write.
    Value,
    /// The tick. `SendMessage BM_GETCHECK` / `BM_SETCHECK`.
    Checked,
    /// `EnableWindow`, and Windows offers no instruction to ask.
    Enabled,
    /// `ShowWindow`, likewise.
    Visible,
    /// `SetCtlColors`, which sets a window's text colour and its background in
    /// **one** instruction — which is why this is one field holding two and not
    /// the `textColor`/`backColor` pair the plan sketched.
    Colors,
    /// `CreateFont` and `WM_SETFONT`.
    Font,
    /// `LoadAndSetImage`, on a `bitmap`.
    Image,
}

impl ControlField {
    /// Whether Windows will say what this field currently is.
    ///
    /// Five of the seven it will not, and each has the same reason: the setter
    /// is a one-way instruction rather than half of a pair. `EnableWindow` has
    /// `IsWindowEnabled` behind it in the API and no NSIS instruction reaches
    /// it, and guessing a `SendMessage` number for the rest would be inventing a
    /// read that returns whatever the control does with an unknown message.
    pub fn readable(self) -> bool {
        matches!(self, ControlField::Value | ControlField::Checked)
    }

    /// Whether `control` has this field.
    ///
    /// `None` is a window this compiler did not create — `getDlgItem(HWNDPARENT,
    /// 2)` is MUI2's Cancel button — and it gets the fields that are true of
    /// every window. The two that depend on how the window was drawn need a
    /// declaration, because a tick on something that is not a checkbox is a
    /// question Windows answers with 0 rather than an error.
    pub fn on(self, control: Option<&Control>) -> bool {
        match (self, control) {
            (ControlField::Checked, control) => control.is_some_and(Control::checkable),
            (ControlField::Image, control) => control.is_some_and(Control::imageable),
            (ControlField::Value, Some(control)) => control.text.is_some(),
            _ => true,
        }
    }

    /// The instruction the write is, for the two errors that have to name it.
    pub fn setter(self) -> &'static str {
        match self {
            ControlField::Value => "SendMessage WM_SETTEXT",
            ControlField::Checked => "SendMessage BM_SETCHECK",
            ControlField::Enabled => "EnableWindow",
            ControlField::Visible => "ShowWindow",
            ControlField::Colors => "SetCtlColors",
            ControlField::Font => "CreateFont",
            ControlField::Image => "LoadAndSetImage",
        }
    }

    /// What has this field, for the error that has to say why this one does not.
    pub fn needs(self) -> &'static str {
        match self {
            ControlField::Checked => "a `checkbox` or a `radioButton`",
            ControlField::Image => "a `bitmap`",
            ControlField::Value => "a control drawn with text",
            _ => "any window",
        }
    }

    /// What the wrong kind would do, which is the half of that error worth
    /// reading: none of the three is refused by Windows, and a control that
    /// quietly ignores an instruction is what this diagnostic is instead of.
    pub fn why(self) -> &'static str {
        match self {
            ControlField::Checked => "Windows answers this one with 0 rather than an error",
            ControlField::Image => {
                "`LoadAndSetImage` on a window drawn with text replaces nothing and says nothing"
            }
            _ => "a control drawn with no text has nothing for this to be",
        }
    }
}

/// The field this name reads, or `None` for a name that is not one.
pub fn control_field(name: &str) -> Option<ControlField> {
    Some(match name {
        "value" => ControlField::Value,
        "checked" => ControlField::Checked,
        "enabled" => ControlField::Enabled,
        "visible" => ControlField::Visible,
        "colors" => ControlField::Colors,
        "font" => ControlField::Font,
        "image" => ControlField::Image,
        _ => return None,
    })
}

/// The field names, for the error that has to list them.
pub const CONTROL_FIELDS: &[&str] = &[
    "value", "checked", "enabled", "visible", "colors", "font", "image",
];

/// The kind this name declares, or `None` for a name that declares no control.
pub fn control(name: &str) -> Option<&'static Control> {
    CONTROLS.iter().find(|control| control.installua == name)
}

/// The kinds, for the error that has to list them.
pub fn names() -> Vec<&'static str> {
    CONTROLS.iter().map(|control| control.installua).collect()
}
