//! Field shapes both lowerers read.
//!
//! There are two lowerers — [`Lowerer`](super::Lowerer) for declarations and
//! `BodyLowerer` for statements — and a handful of shapes belong to both.
//! `colors` is the first: it is a control's field, written as a statement
//! inside a page, *and* a page's own setting and a block's, written as a
//! declaration. MUI2 spends every one of them on the same `SetCtlColors`, so
//! they are one parse, one set of diagnostics and one spelling — and this trait
//! is what lets the two lowerers share it without either one owning the other.
//!
//! The trait is deliberately two methods wide. Anything larger would be an
//! invitation to move real lowering in here, and the two lowerers differ in
//! ways that matter: one has a control-flow graph and the other does not.

use crate::ast::{Expr, TableField};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;

use super::ConstValue;

/// What a shared field parse needs: somewhere to report, and a way to fold a
/// constant.
pub(super) trait Fields {
    fn diags(&mut self) -> &mut Diagnostics;

    fn constant_value(&self, expr: &Expr) -> Option<ConstValue>;

    /// The twin of both lowerers' `bad_value`, spelled once.
    fn bad_field(&mut self, span: Span, what: &str, wanted: &str, note: &str) {
        self.diags().push(
            Diagnostic::error(
                Code::BadFieldValue,
                span,
                format!("`{what}` wants {wanted}"),
            )
            .note(note),
        );
    }

    /// `colors = { text = "000000", background = "FFFFFF" }`.
    ///
    /// One field holding two, because `SetCtlColors` is one instruction that
    /// sets both: `textColor` and `backColor` as separate fields would mean a
    /// write to either silently replacing the other, which is a bug that only
    /// shows up on the screen.
    ///
    /// Named by its caller, because MUI2 reads its colours in pairs through that
    /// same instruction — `installer { headerColors }` and `page.directory
    /// { colors }` are this field one and two levels up.
    fn colours(&mut self, value: &Expr, whole: &str) -> Option<(Option<ir::Arg>, Option<ir::Arg>)> {
        let Expr::Table { fields, span } = value else {
            self.bad_field(
                value.span(),
                whole,
                "a table of two colours",
                "`SetCtlColors` sets the text and the background in one instruction, so the field \
                 that is that instruction is a table of both",
            );
            return None;
        };

        let mut colours: [Option<String>; 2] = [None, None];
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_field(
                    value.span(),
                    whole,
                    "a table of two colours",
                    "the two are named: `{ text = \"000000\", background = \"FFFFFF\" }`",
                );
                continue;
            };
            match name.text.as_str() {
                "text" => colours[0] = self.colour(value, "text"),
                "background" => colours[1] = self.colour(value, "background"),
                other => {
                    let span = name.span;
                    self.diags().push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not one of `{whole}`'s colours"),
                        )
                        .note("the two are `text` and `background`"),
                    );
                }
            }
        }

        // Either half may be left out: `SetCtlColors` reads `""` as the
        // control's own colour, and MUI2 defaults whichever define is absent.
        // Neither is a field that says nothing.
        if colours == [None, None] {
            let span = *span;
            self.diags().push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    format!("`{whole}` has neither `text` nor `background`"),
                )
                .note("`background = \"transparent\"` is the way to leave it unpainted"),
            );
            return None;
        }
        let [text, back] = colours.map(|colour| colour.map(ir::Arg::raw));
        Some((text, back))
    }

    /// One colour: six hexadecimal digits, or `transparent` for a background
    /// that is not painted at all.
    ///
    /// Checked rather than passed through, because `SetCtlColors` reads anything
    /// else as black — a label that vanishes into its own background is the
    /// failure a typo here produces.
    fn colour(&mut self, value: &Expr, field: &str) -> Option<String> {
        let text = match self.constant_value(value) {
            Some(ConstValue::Str(text)) => text,
            _ => String::new(),
        };
        let hex = text.len() == 6 && text.bytes().all(|byte| byte.is_ascii_hexdigit());
        let unpainted = field == "background" && text == "transparent";
        if !hex && !unpainted {
            self.bad_field(
                value.span(),
                field,
                "six hexadecimal digits",
                "`\"FF0000\"` is red, in the order Windows writes it; `background = \
                 \"transparent\"` leaves the background unpainted",
            );
            return None;
        }
        Some(text)
    }
}
