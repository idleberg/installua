//! `runningX64()`, `wow64()` and `nativeMachine("ARM64")` — the conditions of
//! the stock `x64.nsh`.
//!
//! ```lua
//! if runningX64() then setRegView(64) end
//! if nativeMachine("ARM64") then abort("no ARM64 build yet") end
//! ```
//!
//! Calls rather than globals, like `silent()` and `rebootFlag()`: every other
//! runtime question here is one. Each is a LogicLib condition, and LogicLib's
//! `_Name _a _b _t _f` macro shape is already a predicate: it jumps to `_t` or
//! `_f`, with its jump as its last line, so `0` falls through as it does after
//! `IfSilent`. `branch` fuses it into the `if` the same way, through
//! `!insertmacro`, and `${If}` never appears.
//!
//! The macros use LogicLib's own temporary and a balanced `System::Call`
//! stack, so no register is touched and the line is not an opaque site.
//!
//! Left out: `DisableX64FSRedirection`/`EnableX64FSRedirection`, which change
//! state rather than answer, and `GetNativeMachineArchitecture`, whose number
//! `nativeMachine` already compares.

use crate::ast::Expr;
use crate::cfg::Test;
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::resolve::ConstValue;

use super::{BodyLowerer, list};

pub(super) const NAMES: &[&str] = &["runningX64", "wow64", "nativeMachine"];

/// `IMAGE_FILE_MACHINE_*`, the numbers `x64.nsh`'s `IsNative*` compare with.
const MACHINES: &[(&str, &str)] = &[("IA32", "332"), ("AMD64", "34404"), ("ARM64", "43620")];

impl BodyLowerer<'_, '_> {
    /// The call as a `Test`, or `None` once it has been reported.
    pub(super) fn x64(&mut self, name: &str, args: &[Expr], span: Span) -> Option<Test> {
        let (mut macro_args, want) = match name {
            "runningX64" => (vec!["_RunningX64", "\"\"", "\"\""], 0),
            "wow64" => (vec!["_IsWow64", "\"\"", "\"\""], 0),
            _ => (vec!["_IsNativeMachineArchitecture", "\"\""], 1),
        };
        if args.len() != want {
            self.diags.push(Diagnostic::error(
                Code::WrongArity,
                span,
                format!("`{name}` takes {want} argument(s)"),
            ));
            return None;
        }
        if let [machine] = args {
            let machines: Vec<&str> = MACHINES.iter().map(|(name, _)| *name).collect();
            match self.constant(machine) {
                Some(ConstValue::Str(text)) if machines.contains(&text.as_str()) => {
                    macro_args.push(MACHINES.iter().find(|(m, _)| *m == text)?.1);
                }
                _ => {
                    self.bad_value(
                        machine.span(),
                        "nativeMachine",
                        "a machine",
                        &format!("the machines are {}", list(&machines)),
                    );
                    return None;
                }
            }
        }
        self.requires.headers.insert("x64".to_string());
        Some(Test::Predicate {
            name: "!insertmacro".to_string(),
            args: macro_args.into_iter().map(ir::Arg::raw).collect(),
            keywords: Vec::new(),
            more: Vec::new(),
        })
    }
}
