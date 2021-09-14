pub fn indent(s: &str, prefix: &str) -> String {
    s.split("\n")
        .map(|s| format!("{}{}", prefix, s))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_indent() {
    for (i, o) in [
        ("", "p "),
        ("\n\n", "p \np \np "),
        ("a\nb\nc", "p a\np b\np c"),
        ("a\n\nb\n\nc", "p a\np \np b\np \np c"),
    ] {
        assert_eq!(indent(i, "p "), o);
    }
}
