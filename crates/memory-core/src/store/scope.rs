pub fn parse_scope(scope: &str) -> Vec<&str> {
    scope.split('/').filter(|s| !s.is_empty()).collect()
}

pub fn scope_ancestors(scope: &str) -> Vec<String> {
    let segments = parse_scope(scope);
    let mut ancestors = Vec::with_capacity(segments.len() + 1);

    for i in (0..=segments.len()).rev() {
        if i == 0 {
            ancestors.push("/".to_string());
        } else {
            ancestors.push(format!("/{}", segments[..i].join("/")));
        }
    }
    ancestors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_root() {
        assert!(parse_scope("/").is_empty());
    }

    #[test]
    fn parse_segments() {
        assert_eq!(
            parse_scope("/org/acme/project"),
            vec!["org", "acme", "project"]
        );
    }

    #[test]
    fn ancestors_deep() {
        let result = scope_ancestors("/org/acme/project/api");
        assert_eq!(
            result,
            vec![
                "/org/acme/project/api",
                "/org/acme/project",
                "/org/acme",
                "/org",
                "/"
            ]
        );
    }

    #[test]
    fn ancestors_root() {
        let result = scope_ancestors("/");
        assert_eq!(result, vec!["/"]);
    }

    #[test]
    fn ancestors_single() {
        let result = scope_ancestors("/project");
        assert_eq!(result, vec!["/project", "/"]);
    }
}
