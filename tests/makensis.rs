//! `MAKENSIS.*`'s refusals. What it emits is the `build-time` golden's.

use installua::diag::Diagnostics;

fn messages(body: &str) -> Vec<String> {
    let source = format!(
        "attributes {{ name = \"m\", outFile = \"m.exe\" }}\n\
         installer {{ section(\"s\", function()\n{body}\nend) }}\n"
    );
    let mut diags = Diagnostics::new();
    installua::build_with(&source, &installua::Options::default(), &mut diags);
    diags.iter().map(|d| d.message.clone()).collect()
}

#[test]
fn an_install_time_value_cannot_reach_the_build_machine() {
    assert_eq!(
        messages("local x = \"a\"\nMAKENSIS.echo(x)\nMAKENSIS.system(INSTDIR)"),
        [
            "`MAKENSIS.echo` runs on the build machine, and this is known only at install time",
            "`MAKENSIS.system` runs on the build machine, and this is known only at install time",
        ]
    );
}

#[test]
fn a_literal_dollar_is_the_command_s_and_not_a_warning() {
    assert_eq!(
        messages("MAKENSIS.system(\"echo $HOME\")"),
        Vec::<String>::new()
    );
}

#[test]
fn echo_answers_nothing_and_an_unknown_method_is_named() {
    assert_eq!(
        messages("local x = MAKENSIS.echo(\"hi\")\nMAKENSIS.nope(\"x\")"),
        [
            "`MAKENSIS.echo` answers 0 value(s), and 1 are being bound",
            "`MAKENSIS` has no `nope`",
        ]
    );
}
