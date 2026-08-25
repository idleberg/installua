//! The line map: where every line of the `.nsi` came from.
//!
//! **The normal mechanism is unavailable, and that is what makes this a
//! file.** Every other transpiler pushes its mapping *into* the artifact and
//! lets the downstream tool translate. NSIS has the read half and not the write
//! half — `${__LINE__}` expands the compiler's position into script data, and
//! `!define /redef __LINE__ 500` is silently ignored — so the map cannot travel
//! with the output, and something has to sit between `makensis` and the user.
//! That something is `installua build`, which is why this compiler is a build
//! tool rather than a program that emits a file.
//!
//! The population being mapped is small, and that is what makes it affordable.
//! The premise is that a well-formed Installua program does not reach a
//! `makensis` diagnostic at all, so what survives is the escape hatches: a
//! `raw` block, an undeclared header macro, a plugin call, a missing `File`
//! source. Four categories, not a general source map.

use crate::diag::Span;

/// Where one line of the output came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Ordinary lowering: an Installua statement or expression.
    User(Span),
    /// Inside a `raw [[ … ]]` block — unchecked by design, and the message
    /// says so rather than pretending the compiler vouched for it.
    Raw(Span),
    /// The compiler's own line: an `!include`, a `${Using:StrFunc}`, a label, a
    /// caller-save. **No span**, because inventing the nearest one would point
    /// a user at code that is not responsible — and a `makensis` complaint
    /// about one of these is by definition a compiler bug.
    Emitted(&'static str),
}

/// One entry per line of the emitted `.nsi`, in order.
#[derive(Clone, Debug, Default)]
pub struct LineMap {
    lines: Vec<Origin>,
}

impl LineMap {
    pub fn push(&mut self, origin: Origin) {
        self.lines.push(origin);
    }

    /// The origin of a 1-based output line, as `makensis` reports them.
    pub fn origin(&self, line: usize) -> Option<&Origin> {
        self.lines.get(line.checked_sub(1)?)
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Origin> {
        self.lines.iter()
    }
}
