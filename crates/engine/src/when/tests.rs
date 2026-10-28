use super::expr::{self, Chip};
use super::keys::{self, Context};
use super::suggest::{self, ItemKind};
use super::*;

fn p(src: &str) -> Expr {
    parse(src).unwrap_or_else(|e| panic!("{src:?}: {e}")).unwrap_or_else(|| panic!("{src:?} is blank"))
}

fn e(src: &str) -> ParseError {
    match parse(src) {
        Err(e) => e,
        Ok(x) => panic!("{src:?} parsed: {x:?}"),
    }
}

fn holds(src: &str, cx: &Context) -> bool {
    p(src).eval(cx)
}

fn brush() -> Context {
    Context::default().with("tool", "brush")
}

#[test]
fn blank_is_no_condition() {
    assert_eq!(parse("").unwrap(), None);
    assert_eq!(parse("  \t ").unwrap(), None);
}

#[test]
fn parses_every_operator() {
    assert_eq!(p("tool == brush").text(), "tool == brush");
    assert_eq!(p("tool != crop").text(), "tool != crop");
    assert_eq!(p("textFocus").text(), "textFocus");
    assert_eq!(p("!textFocus").text(), "!textFocus");
    assert_eq!(p("tool==brush&&hasDocument").text(), "tool == brush && hasDocument");
    assert_eq!(p("tool == brush || tool == pencil").text(), "tool == brush || tool == pencil");
    // && binds tighter than ||, parentheses override it
    assert_eq!(p("hasDocument || hasSelection && textFocus").text(), "hasDocument || hasSelection && textFocus");
    assert_eq!(p("(hasDocument || hasSelection) && textFocus").text(), "(hasDocument || hasSelection) && textFocus");
    assert_eq!(p("((tool == brush))").text(), "tool == brush");
    assert_eq!(p("textFocus == false").text(), "!textFocus");
    assert_eq!(p("textFocus != false").text(), "textFocus");
    assert_eq!(p("tool == \"brush\"").text(), "tool == brush");
    assert_eq!(p("!dialogOpen && tool == brush").text(), "!dialogOpen && tool == brush");
}

#[test]
fn negation_is_first_class() {
    assert_eq!(p("!(tool == brush)").text(), "!(tool == brush)");
    assert_eq!(p("!(tool == brush && hasSelection)").text(), "!(tool == brush && hasSelection)");
    assert_eq!(p("!!textFocus").text(), "!!textFocus");
    assert_eq!(p("!textFocus && !dialogOpen").text(), "!textFocus && !dialogOpen");
    assert_eq!(p("!(textFocus || dialogOpen)").text(), "!(textFocus || dialogOpen)");
    // the canonical text parses back to the same condition
    for src in ["!(tool == brush)", "!!textFocus", "!(hasPath)", "!(hasDocument || hasSelection) && tool != move"] {
        assert_eq!(p(&p(src).text()), p(src), "{src}");
    }
}

#[test]
fn evaluation_of_negations() {
    let mut cx = brush();
    assert!(holds("tool == brush", &cx));
    assert!(!holds("tool != brush", &cx));
    assert!(!holds("!(tool == brush)", &cx));
    assert!(holds("!(tool == move)", &cx));
    assert!(holds("!!(tool == brush)", &cx));
    assert!(holds("!textFocus", &cx));
    assert!(!holds("textFocus", &cx));
    cx.set_flag(keys::TEXT_FOCUS, true);
    assert!(!holds("!textFocus", &cx));
    assert!(holds("!textFocus || tool == brush", &cx));
    assert!(!holds("!(textFocus || tool == brush)", &cx));
    assert!(holds("!(textFocus && tool == move)", &cx));
}

#[test]
fn evaluation_of_and_or() {
    let cx = brush().with("hasSelection", "true");
    assert!(holds("tool == brush && hasSelection", &cx));
    assert!(!holds("tool == brush && hasPath", &cx));
    assert!(holds("tool == move || hasSelection", &cx));
    assert!(holds("(tool == move || tool == brush) && !hasPath", &cx));
    assert!(!holds("tool == move || tool == crop", &cx));
}

#[test]
fn bad_input_reports_where_and_why() {
    assert!(e("tol == brush").message.contains("Unknown key “tol”, did you mean “tool”"));
    assert_eq!(e("tol == brush").at, 0..3);
    assert!(e("tool == brsh").message.contains("did you mean “brush”"));
    assert!(e("tool == nonsense").message.contains("one of: move, artboard"));
    assert!(e("tool = brush").message.contains("=="));
    assert!(e("a & b").message.contains("&&"));
    assert!(e("tool | brush").message.contains("||"));
    assert!(e("tool").message.contains("needs a value"));
    assert!(e("textFocus == maybe").message.contains("true or false"));
    assert!(e("tool == brush and hasPath").message.contains("Unexpected “and”"));
    assert!(e("and").message.contains("&&"));
    assert!(e("tool == brush )").message.contains("Unexpected “)”"));
    assert!(e(") tool").message.contains("no ("));
    assert!(e("(tool == brush").message.contains("missing"));
    assert!(e("tool == brush $").message.contains("Unexpected character “$”"));
    assert!(e("tool == \"brush").message.contains("quote"));
    assert!(e("tool == brush &&").incomplete);
    assert!(e("tool ==").incomplete);
    assert!(e("!").incomplete);
    assert!(!e("tol").incomplete);
    assert!(e(&"x".repeat(parse::MAX_LEN + 1)).message.contains("too long"));
}

#[test]
fn bad_input_never_panics() {
    let nested = format!("{}textFocus{}", "(".repeat(150), ")".repeat(150));
    assert!(e(&nested).message.contains("Too many"));
    assert!(e(&"(".repeat(5000)).message.contains("too long"));
    assert!(e(&format!("{}textFocus", "!".repeat(100))).message.contains("Too many"));
    for junk in [
        "é",
        "日本語 == 語",
        "tool ==\u{0}",
        "&&",
        "||",
        "==",
        "!=",
        "!=!=",
        "()",
        "(!)",
        "!(",
        "'",
        "\"",
        "tool == \u{1F600}",
        "tool\u{301} == brush",
    ] {
        let _ = parse(junk);
        let _ = suggest::suggest(junk, junk.len());
        for i in 0..=junk.len() {
            let _ = suggest::suggest(junk, i);
        }
    }
}

#[test]
fn chips_say_negation_clearly() {
    let atoms = |src: &str| -> Vec<String> {
        p(src)
            .chips()
            .into_iter()
            .map(|c| match c {
                Chip::Atom(s) => s,
                Chip::Word(w) => w.to_string(),
                Chip::Open => "(".into(),
                Chip::Close => ")".into(),
            })
            .collect()
    };
    assert_eq!(atoms("tool == brush"), ["Brush Tool"]);
    assert_eq!(atoms("tool != brush"), ["Not Brush Tool"]);
    assert_eq!(atoms("!(tool == brush)"), ["Not Brush Tool"]);
    assert_eq!(atoms("!(tool != brush)"), ["Brush Tool"]);
    assert_eq!(atoms("textFocus"), ["Typing"]);
    assert_eq!(atoms("!textFocus"), ["Not Typing"]);
    assert_eq!(atoms("!!textFocus"), ["Typing"]);
    assert_eq!(atoms("tool == brush && hasSelection"), ["Brush Tool", "and", "Selection Active"]);
    assert_eq!(atoms("tool == brush || !textFocus"), ["Brush Tool", "or", "Not Typing"]);
    assert_eq!(atoms("(tool == brush || tool == pencil) && !textFocus"), ["(", "Brush Tool", "or", "Pencil Tool", ")", "and", "Not Typing"]);
    assert_eq!(atoms("!(tool == brush && hasSelection)"), ["Not", "(", "Brush Tool", "and", "Selection Active", ")"]);
}

#[test]
fn two_conditions_overlap_unless_they_exclude_each_other() {
    let o = |a: &str, b: &str| expr::overlap(parse(a).unwrap().as_ref(), parse(b).unwrap().as_ref()).is_some();
    // no condition always applies
    assert!(o("", "tool == brush"));
    assert!(o("", ""));
    assert!(o("tool == brush", "hasSelection"));
    assert!(!o("tool == brush", "tool == move"));
    assert!(!o("tool == brush", "tool != brush"));
    assert!(!o("textFocus", "!textFocus"));
    // negations: not brush still includes the move tool
    assert!(o("tool != brush", "tool == move"));
    assert!(o("!(tool == brush)", "tool != move"));
    assert!(!o("!(tool == brush)", "tool == brush"));
    assert!(!o("!(tool == brush || tool == move)", "tool == move"));
    assert!(o("!(tool == brush && hasSelection)", "tool == brush"), "brush without a selection is outside the negated group");
    assert!(!o("!(tool == brush && hasSelection)", "tool == brush && hasSelection"));
    // keys that depend on others
    assert!(!o("hasSelection", "!hasDocument"));
    assert!(!o("layerKind == text", "!hasDocument"));
    assert!(!o("textFocus", "widgetFocus"));
    assert!(o("layerKind == text", "hasPath"));
}

#[test]
fn a_condition_that_can_never_hold_is_noticed() {
    assert!(!expr::satisfiable(&p("tool == brush && tool == move")));
    assert!(!expr::satisfiable(&p("textFocus && !textFocus")));
    assert!(!expr::satisfiable(&p("hasSelection && !hasDocument")));
    assert!(expr::satisfiable(&p("tool == brush || tool == move")));
    assert!(expr::satisfiable(&p("!(tool == brush)")));
}

#[test]
fn the_witness_names_the_shared_context() {
    let a = p("tool == brush");
    let b = p("!textFocus");
    let cx = expr::overlap(Some(&a), Some(&b)).unwrap();
    assert_eq!(expr::describe(&cx, Some(&a), Some(&b)), "Brush Tool, Not Typing");
}

#[test]
fn every_key_and_value_is_typable() {
    for (i, k) in keys::KEYS.iter().enumerate() {
        assert_eq!(keys::key_index(k.name), Some(i));
        assert!(!k.desc.is_empty());
        if k.is_bool() {
            p(k.name);
            p(&format!("!{}", k.name));
        }
        for v in k.values() {
            p(&format!("{} == {}", k.name, v.name));
            p(&format!("{} != {}", k.name, v.name));
        }
    }
    assert_eq!((keys::TOOL, keys::FULLSCREEN), (keys::key_index("tool").unwrap(), keys::key_index("fullscreen").unwrap()));
    assert_eq!(keys::KEYS.len(), keys::FULLSCREEN + 1);
}

#[test]
fn every_tool_is_a_value_of_tool() {
    for (id, label, _) in crate::shell_commands::TOOLS {
        let name = id.strip_prefix("tools.select.").unwrap();
        let i = keys::value_index(keys::TOOL, name).unwrap_or_else(|| panic!("{id} is not a value of tool"));
        assert_eq!(keys::KEYS[keys::TOOL].values()[i].chip, *label);
    }
    assert_eq!(keys::KEYS[keys::TOOL].values().len(), crate::shell_commands::TOOLS.len());
}

fn labels(text: &str) -> Vec<String> {
    suggest::suggest(text, text.len()).items.into_iter().map(|i| i.label).collect()
}

#[test]
fn autocomplete_offers_keys_and_negated_forms_first() {
    let all = labels("");
    assert_eq!(all.first().map(String::as_str), Some("tool"));
    for want in ["layerKind", "textFocus", "!textFocus", "!hasSelection", "!", "!(", "("] {
        assert!(all.iter().any(|l| l == want), "{want} missing from {all:?}");
    }
    assert!(!all.iter().any(|l| l == "!tool"), "an enumeration key is negated with !=, not with !");
    assert!(all.iter().all(|l| !l.is_empty()));
    assert!(suggest::suggest("", 0).items.iter().all(|i| !i.detail.is_empty()), "each suggestion has a description");
}

#[test]
fn autocomplete_filters_as_you_type() {
    assert_eq!(labels("to").first().map(String::as_str), Some("tool"), "names that start with the word come first");
    assert_eq!(&labels("te")[..2], ["textFocus", "!textFocus"]);
    assert!(labels("nonsense").is_empty());
    // after && and ||, after ( and after !, a key is expected
    for t in ["tool == brush && ", "tool == brush || ", "(", "!", "(!", "!("] {
        assert!(labels(t).contains(&"tool".to_string()), "{t}: {:?}", labels(t));
    }
    assert!(labels("!").contains(&"hasDocument".to_string()));
}

#[test]
fn autocomplete_follows_the_grammar() {
    // after an enumeration key: the comparison, both polarities
    assert_eq!(labels("tool "), ["==", "!="]);
    assert_eq!(labels("tool !"), ["!="]);
    assert_eq!(labels("tool ="), ["=="]);
    // after the operator: that key's values
    assert_eq!(labels("layerKind == ").first().map(String::as_str), Some("none"));
    assert_eq!(labels("layerKind != sm"), ["smart"]);
    assert!(labels("tool != ").contains(&"crop".to_string()));
    // after a value or an on / off key: and, or
    assert_eq!(labels("tool == brush "), ["&&", "||"]);
    assert_eq!(labels("textFocus "), ["&&", "||"]);
    assert_eq!(labels("!textFocus &"), ["&&"]);
    // a close parenthesis only while one is open
    assert_eq!(labels("(tool == brush "), ["&&", "||", ")"]);
    assert_eq!(labels("!(tool == brush "), ["&&", "||", ")"]);
    // nonsense gets nothing
    assert!(labels("tool == brush tool").is_empty());
    assert!(labels("tool @").is_empty());
    assert!(labels("foo ").is_empty());
}

#[test]
fn accepting_a_suggestion_replaces_the_word_and_moves_on() {
    let take = |text: &str, label: &str| {
        let s = suggest::suggest(text, text.len());
        let item = s.items.iter().find(|i| i.label == label).unwrap_or_else(|| panic!("{label} not offered at {text:?}"));
        suggest::apply(text, &s, item)
    };
    assert_eq!(take("to", "tool"), ("tool ".to_string(), 5));
    assert_eq!(take("tool ", "!="), ("tool != ".to_string(), 8));
    assert_eq!(take("tool != c", "crop").0, "tool != crop ");
    assert_eq!(take("tool == brush && te", "!textFocus").0, "tool == brush && !textFocus ");
    assert_eq!(take("!", "textFocus").0, "!textFocus ");
    assert_eq!(take("", "!(").0, "!(");
    // a whole chain, one Tab at a time, is a valid condition
    let mut text = String::new();
    for label in ["tool", "!=", "crop", "&&", "!textFocus"] {
        text = take(&text, label).0;
    }
    assert_eq!(p(&text).text(), "tool != crop && !textFocus");
    assert!(suggest::suggest("to", 2).items.iter().all(|i| i.kind == ItemKind::Key));
}

#[test]
fn the_context_checks_itself() {
    assert!(Context::default().consistent());
    assert!(!Context::default().with("hasSelection", "true").consistent());
    assert!(Context::default().with("hasDocument", "true").with("hasSelection", "true").consistent());
    assert!(!Context::default().with("layerKind", "text").consistent());
    assert!(!Context::default().with("textFocus", "true").with("widgetFocus", "true").consistent());
    // unknown names leave it alone
    assert_eq!(Context::default().with("nope", "x"), Context::default());
}
