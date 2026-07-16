use std::collections::BTreeSet;

use crate::models::AuthObservation;

pub fn detect_auth_indicators(text: &str) -> Vec<String> {
    let lowered = text.to_ascii_lowercase();
    let mut indicators = BTreeSet::new();

    for (needle, label) in [
        ("authorization", "authorization_header"),
        ("bearer", "bearer_token"),
        ("jwt", "jwt"),
        ("oauth", "oauth"),
        ("oidc", "oidc"),
        ("openid", "oidc"),
        ("saml", "saml"),
        ("session", "session_cookie"),
        ("cookie", "session_cookie"),
        ("refresh_token", "refresh_token"),
        ("csrf", "csrf"),
        ("x-api-key", "api_key"),
        ("api_key", "api_key"),
        ("token", "token_reference"),
        ("login", "login_flow"),
        ("sso", "sso"),
    ] {
        if lowered.contains(needle) {
            indicators.insert(label.to_string());
        }
    }

    indicators.into_iter().collect()
}

pub fn detect_auth_indicators_in_texts(texts: &[String]) -> Vec<String> {
    let mut indicators = BTreeSet::new();
    for text in texts {
        for indicator in detect_auth_indicators(text) {
            indicators.insert(indicator);
        }
    }
    indicators.into_iter().collect()
}

pub fn classify_auth_type(indicators: &[String]) -> Option<String> {
    let values = indicators
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();

    if values
        .iter()
        .any(|value| matches!(value.as_str(), "jwt" | "bearer_token"))
    {
        return Some("jwt_bearer".to_string());
    }
    if values
        .iter()
        .any(|value| matches!(value.as_str(), "oauth" | "oidc" | "sso"))
    {
        return Some("oauth_oidc".to_string());
    }
    if values.iter().any(|value| value == "saml") {
        return Some("saml".to_string());
    }
    if values.iter().any(|value| value == "api_key") {
        return Some("api_key".to_string());
    }
    if values
        .iter()
        .any(|value| matches!(value.as_str(), "session_cookie" | "csrf"))
    {
        return Some("session_cookie".to_string());
    }
    if values
        .iter()
        .any(|value| matches!(value.as_str(), "token_reference" | "authorization_header"))
    {
        return Some("token_header".to_string());
    }

    None
}

pub fn build_auth_observation(
    asset: &str,
    texts: &[String],
    evidence: &[String],
) -> Option<AuthObservation> {
    let indicators = detect_auth_indicators_in_texts(texts);
    let auth_type = classify_auth_type(&indicators)?;

    let mut observation_evidence = evidence.to_vec();
    if observation_evidence.is_empty() {
        observation_evidence = texts
            .iter()
            .filter(|text| !detect_auth_indicators(text).is_empty())
            .take(6)
            .map(|text| format!("Auth-related content matched in '{text}'"))
            .collect();
    }

    Some(AuthObservation {
        asset: asset.to_string(),
        auth_type,
        indicators: indicators.clone(),
        confidence: auth_confidence(&indicators),
        evidence: observation_evidence,
    })
}

fn auth_confidence(indicators: &[String]) -> f32 {
    let base = 0.65 + (indicators.len() as f32 * 0.05);
    ((base.min(0.95)) * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::{build_auth_observation, classify_auth_type, detect_auth_indicators};

    #[test]
    fn auth_keyword_detection_finds_common_markers() {
        let indicators = detect_auth_indicators(
            "Authorization: Bearer token, refresh_token handling, csrf checks, session cookie",
        );
        assert!(indicators
            .iter()
            .any(|value| value == "authorization_header"));
        assert!(indicators.iter().any(|value| value == "bearer_token"));
        assert!(indicators.iter().any(|value| value == "refresh_token"));
        assert!(indicators.iter().any(|value| value == "csrf"));
        assert!(indicators.iter().any(|value| value == "session_cookie"));
    }

    #[test]
    fn jwt_and_oauth_recognition_is_deterministic() {
        let auth_type = classify_auth_type(&[
            "authorization_header".to_string(),
            "bearer_token".to_string(),
            "jwt".to_string(),
            "oauth".to_string(),
        ])
        .expect("auth type should be detected");
        assert_eq!(auth_type, "jwt_bearer");

        let observation = build_auth_observation(
            "login.example.com",
            &[
                "OAuth callback".to_string(),
                "OIDC issuer".to_string(),
                "SSO login".to_string(),
            ],
            &[],
        )
        .expect("observation should be created");
        assert_eq!(observation.auth_type, "oauth_oidc");
    }
}
