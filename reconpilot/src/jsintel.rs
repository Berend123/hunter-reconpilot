use std::collections::BTreeSet;

use regex::Regex;

use crate::{
    auth, classifiers,
    models::{AssetRole, JsObservation},
};

pub fn analyze_javascript(js_file: &str, content: &str) -> JsObservation {
    let discovered_endpoints = extract_endpoints(content);
    let discovered_auth_indicators = auth::detect_auth_indicators(content);
    let discovered_feature_flags = extract_feature_flags(content);

    let mut role_inputs = discovered_endpoints.clone();
    role_inputs.push(js_file.to_string());
    role_inputs.extend(discovered_feature_flags.clone());
    let discovered_roles = classifiers::classify_roles(&role_inputs, &[])
        .into_iter()
        .filter(|role| *role != AssetRole::Unknown)
        .map(|role| role.as_str().to_string())
        .collect::<Vec<_>>();

    let mut evidence = BTreeSet::new();
    for endpoint in discovered_endpoints.iter().take(8) {
        evidence.insert(format!("JavaScript referenced endpoint '{endpoint}'"));
    }
    for indicator in &discovered_auth_indicators {
        evidence.insert(format!(
            "JavaScript content matched auth indicator '{indicator}'"
        ));
    }
    for flag in &discovered_feature_flags {
        evidence.insert(format!("JavaScript content matched feature flag '{flag}'"));
    }
    for environment in extract_environment_references(content) {
        evidence.insert(format!(
            "JavaScript content matched environment reference '{environment}'"
        ));
    }

    JsObservation {
        js_file: js_file.to_string(),
        discovered_endpoints,
        discovered_roles,
        discovered_auth_indicators,
        discovered_feature_flags,
        evidence: evidence.into_iter().collect(),
    }
}

pub fn extract_endpoints(content: &str) -> Vec<String> {
    let absolute_re =
        Regex::new(r#"https?://[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]+"#).expect("valid regex");
    let relative_re = Regex::new(r#"["'`](/[-A-Za-z0-9_./{}?=&%]+)["'`]"#).expect("valid regex");

    let mut endpoints = BTreeSet::new();
    for capture in absolute_re.find_iter(content) {
        let candidate = sanitize_endpoint(capture.as_str());
        if looks_like_endpoint(&candidate) {
            endpoints.insert(candidate);
        }
    }
    for capture in relative_re.captures_iter(content) {
        let candidate = sanitize_endpoint(&capture[1]);
        if looks_like_endpoint(&candidate) {
            endpoints.insert(candidate);
        }
    }

    endpoints.into_iter().collect()
}

pub fn extract_feature_flags(content: &str) -> Vec<String> {
    let lowered = content.to_ascii_lowercase();
    let mut values = BTreeSet::new();
    for needle in [
        "featureflag",
        "feature_flag",
        "beta",
        "experimental",
        "canary",
        "debug",
        "adminonly",
        "privileged",
    ] {
        if lowered.contains(needle) {
            values.insert(needle.to_string());
        }
    }
    values.into_iter().collect()
}

pub fn extract_environment_references(content: &str) -> Vec<String> {
    let lowered = content.to_ascii_lowercase();
    let mut values = BTreeSet::new();
    for needle in [
        "staging",
        "stage",
        "dev",
        "development",
        "sandbox",
        "internal",
        "prod",
        "production",
    ] {
        if lowered.contains(needle) {
            values.insert(needle.to_string());
        }
    }
    values.into_iter().collect()
}

fn looks_like_endpoint(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    if lowered.ends_with(".css")
        || lowered.ends_with(".png")
        || lowered.ends_with(".jpg")
        || lowered.ends_with(".jpeg")
        || lowered.ends_with(".svg")
        || lowered.ends_with(".woff")
        || lowered.ends_with(".woff2")
        || lowered.ends_with(".map")
    {
        return false;
    }

    if lowered.contains("/api")
        || lowered.contains("/graphql")
        || lowered.contains("/swagger")
        || lowered.contains("/openapi")
        || lowered.contains("/admin")
        || lowered.contains("/internal")
        || lowered.contains("/export")
        || lowered.contains("/backup")
        || lowered.contains("/upload")
        || lowered.contains("/users")
        || lowered.contains("/billing")
        || lowered.contains("/payments")
    {
        return true;
    }

    value
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count()
        >= 2
}

fn sanitize_endpoint(value: &str) -> String {
    value
        .trim_matches(|character| matches!(character, '"' | '\'' | '`' | ',' | ';'))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{analyze_javascript, extract_endpoints};

    #[test]
    fn js_endpoint_extraction_finds_hidden_routes() {
        let endpoints = extract_endpoints(
            r#"
            const admin = "/admin/users";
            const api = "/api/v1/accounts/{account_id}";
            const gql = "https://app.example.com/graphql";
            "#,
        );
        assert!(endpoints.iter().any(|value| value == "/admin/users"));
        assert!(endpoints
            .iter()
            .any(|value| value == "/api/v1/accounts/{account_id}"));
        assert!(endpoints
            .iter()
            .any(|value| value == "https://app.example.com/graphql"));
    }

    #[test]
    fn js_analysis_captures_auth_and_feature_flags() {
        let observation = analyze_javascript(
            "app.js",
            r#"
            const graphql = "/graphql";
            const token = localStorage.getItem("jwt");
            const featureFlag = "adminOnlyBeta";
            "#,
        );
        assert!(observation
            .discovered_auth_indicators
            .iter()
            .any(|value| value == "jwt"));
        assert!(observation
            .discovered_feature_flags
            .iter()
            .any(|value| value.contains("feature")));
        assert!(observation
            .discovered_endpoints
            .iter()
            .any(|value| value == "/graphql"));
    }
}
