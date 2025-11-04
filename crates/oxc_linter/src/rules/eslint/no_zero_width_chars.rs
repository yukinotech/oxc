use oxc_ast::AstKind;
use oxc_diagnostics::OxcDiagnostic;
use oxc_macros::declare_oxc_lint;
use oxc_semantic::AstNode;
use oxc_span::Span;

use crate::{context::LintContext, rule::Rule};

fn no_zero_width_chars_diagnostic(span: Span, ch: char) -> OxcDiagnostic {
    let code_point = ch as u32;
    let message = format!("Unexpected zero-width character U+{code_point:04X}");
    OxcDiagnostic::warn(message)
        .with_help("Remove the zero-width character to avoid hidden bugs")
        .with_label(span)
}

#[derive(Debug, Default, Clone)]
pub struct NoZeroWidthChars;

declare_oxc_lint!(
    /// ### What it does
    ///
    /// Disallows the use of zero-width characters inside identifiers, string literals, and property keys.
    ///
    /// ### Why is this bad?
    ///
    /// Zero-width characters are invisible to readers and can make debugging difficult by changing runtime
    /// behaviour without any obvious visual cue. They are often introduced accidentally or as part of copy
    /// and paste operations.
    ///
    /// ### Examples
    ///
    /// Examples of **incorrect** code for this rule:
    /// ```javascript
    /// const f\u200Boot = 1;
    /// const text = "\u200B";
    /// const obj = { pr\uFEFFop: 1 };
    /// ```
    ///
    /// Examples of **correct** code for this rule:
    /// ```javascript
    /// const foot = 1;
    /// const text = "regular";
    /// const obj = { prop: 1 };
    /// ```
    NoZeroWidthChars,
    eslint,
    correctness
);

impl Rule for NoZeroWidthChars {
    fn run<'a>(&self, node: &AstNode<'a>, ctx: &LintContext<'a>) {
        match node.kind() {
            AstKind::BindingIdentifier(ident) => {
                report_if_contains_zero_width(ctx, ident.span, ident.name.as_str())
            }
            AstKind::IdentifierReference(ident) => {
                report_if_contains_zero_width(ctx, ident.span, ident.name.as_str());
            }
            AstKind::IdentifierName(ident) => {
                report_if_contains_zero_width(ctx, ident.span, ident.name.as_str())
            }
            AstKind::LabelIdentifier(ident) => {
                report_if_contains_zero_width(ctx, ident.span, ident.name.as_str())
            }
            AstKind::PrivateIdentifier(ident) => {
                report_if_contains_zero_width(ctx, ident.span, ident.name.as_str())
            }
            AstKind::JSXIdentifier(ident) => {
                report_if_contains_zero_width(ctx, ident.span, ident.name.as_str())
            }
            AstKind::StringLiteral(literal) => {
                report_if_contains_zero_width(ctx, literal.span, literal.value.as_str())
            }
            AstKind::TemplateElement(element) => {
                if let Some(cooked) = &element.value.cooked {
                    report_if_contains_zero_width(ctx, element.span, cooked.as_str());
                }
            }
            AstKind::JSXText(text) => {
                report_if_contains_zero_width(ctx, text.span, text.value.as_str())
            }
            _ => {}
        }
    }
}

fn report_if_contains_zero_width(ctx: &LintContext, span: Span, value: &str) {
    if let Some(ch) = find_zero_width_char(value) {
        ctx.diagnostic(no_zero_width_chars_diagnostic(span, ch));
    }
}

fn find_zero_width_char(value: &str) -> Option<char> {
    value.chars().find(|ch| matches!(*ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}'))
}

#[expect(clippy::invisible_characters)]
#[test]
fn test() {
    use crate::tester::Tester;

    let pass =
        vec!["const value = 'normal';", "const obj = { prop: 1 };", r#"const text = "\u0020";"#];

    let fail = vec![
        "const f\u{200C}oo = 1;",
        "const obj = { pr\u{200D}op: 1 };",
        r#"const text = "\u200B";"#,
        "const hidden = \"a\u{200D}b\";",
    ];

    Tester::new(NoZeroWidthChars::NAME, NoZeroWidthChars::PLUGIN, pass, fail).test_and_snapshot();
}
