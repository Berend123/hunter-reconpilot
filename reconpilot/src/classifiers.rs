use std::collections::{BTreeMap, BTreeSet};

use url::Url;

use crate::models::{AssetRole, EnvironmentType, SemanticTag};

pub fn classify_environment_tags(texts: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for text in texts {
        let lowered = text.to_ascii_lowercase();
        for (needle, environment, confidence) in [
            ("prod", EnvironmentType::Production, 0.7),
            ("production", EnvironmentType::Production, 0.9),
            ("live", EnvironmentType::Production, 0.6),
            ("staging", EnvironmentType::Staging, 0.9),
            ("stage", EnvironmentType::Staging, 0.7),
            ("stg", EnvironmentType::Staging, 0.6),
            ("dev", EnvironmentType::Development, 0.7),
            ("development", EnvironmentType::Development, 0.9),
            ("test", EnvironmentType::Testing, 0.7),
            ("qa", EnvironmentType::Testing, 0.7),
            ("sandbox", EnvironmentType::Testing, 0.8),
            ("internal", EnvironmentType::Internal, 0.9),
            ("intranet", EnvironmentType::Internal, 0.9),
            ("corp", EnvironmentType::Internal, 0.7),
            ("legacy", EnvironmentType::Legacy, 0.9),
            ("old", EnvironmentType::Legacy, 0.6),
            ("deprecated", EnvironmentType::Legacy, 0.9),
        ] {
            if lowered.contains(needle) {
                push_tag(
                    &mut tags,
                    SemanticTag {
                        tag: environment.as_str().to_string(),
                        category: "environment".to_string(),
                        confidence,
                        evidence: vec![format!("Matched keyword '{needle}' in '{text}'")],
                    },
                );
            }
        }
    }

    tags
}

pub fn classify_environments(texts: &[String]) -> Vec<EnvironmentType> {
    let mut values = classify_environment_tags(texts)
        .into_iter()
        .filter_map(|tag| environment_from_tag(&tag.tag))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();

    if values.is_empty() {
        values.push(EnvironmentType::Unknown);
    }

    values
}

pub fn classify_role_tags(texts: &[String], technologies: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for text in texts {
        let lowered = text.to_ascii_lowercase();
        for (needle, role, confidence) in [
            ("auth", AssetRole::Authentication, 0.7),
            ("login", AssetRole::Authentication, 0.9),
            ("sso", AssetRole::Authentication, 0.9),
            ("oauth", AssetRole::Authentication, 0.9),
            ("saml", AssetRole::Authentication, 0.9),
            ("admin", AssetRole::AdminDashboard, 0.9),
            ("dashboard", AssetRole::AdminDashboard, 0.8),
            ("console", AssetRole::AdminDashboard, 0.8),
            ("panel", AssetRole::AdminDashboard, 0.7),
            ("api", AssetRole::ApiGateway, 0.7),
            ("gateway", AssetRole::ApiGateway, 0.8),
            ("graphql", AssetRole::ApiGateway, 0.9),
            ("storage", AssetRole::Storage, 0.8),
            ("bucket", AssetRole::Storage, 0.8),
            ("s3", AssetRole::Storage, 0.8),
            ("blob", AssetRole::Storage, 0.8),
            ("docs", AssetRole::Documentation, 0.8),
            ("swagger", AssetRole::Documentation, 0.9),
            ("openapi", AssetRole::Documentation, 0.9),
            ("redoc", AssetRole::Documentation, 0.9),
            ("analytics", AssetRole::Analytics, 0.8),
            ("metrics", AssetRole::Analytics, 0.7),
            ("customer", AssetRole::CustomerApp, 0.6),
            ("portal", AssetRole::CustomerApp, 0.6),
            ("app", AssetRole::CustomerApp, 0.4),
        ] {
            if lowered.contains(needle) {
                push_tag(
                    &mut tags,
                    SemanticTag {
                        tag: role.as_str().to_string(),
                        category: "role".to_string(),
                        confidence,
                        evidence: vec![format!("Matched keyword '{needle}' in '{text}'")],
                    },
                );
            }
        }
    }

    for technology in technologies {
        let lowered = technology.to_ascii_lowercase();
        let tag = if contains_any(&lowered, &["grafana", "prometheus", "datadog", "sentry"]) {
            Some((AssetRole::Monitoring, 0.9))
        } else if contains_any(&lowered, &["kibana", "elasticsearch"]) {
            Some((AssetRole::Logging, 0.9))
        } else if contains_any(&lowered, &["jenkins", "gitlab"]) || lowered == "ci" {
            Some((AssetRole::CICD, 0.9))
        } else if lowered.contains("graphql") || lowered.contains("gateway") {
            Some((AssetRole::ApiGateway, 0.8))
        } else if contains_any(
            &lowered,
            &["next.js", "express", "spring", "laravel", "django", "rails"],
        ) {
            Some((AssetRole::CustomerApp, 0.6))
        } else {
            None
        };

        if let Some((role, confidence)) = tag {
            push_tag(
                &mut tags,
                SemanticTag {
                    tag: role.as_str().to_string(),
                    category: "role".to_string(),
                    confidence,
                    evidence: vec![format!(
                        "Technology '{technology}' suggests role {}",
                        role.as_str()
                    )],
                },
            );
        }
    }

    tags
}

pub fn classify_roles(texts: &[String], technologies: &[String]) -> Vec<AssetRole> {
    let mut roles = classify_role_tags(texts, technologies)
        .into_iter()
        .filter_map(|tag| role_from_tag(&tag.tag))
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();

    if roles.is_empty() {
        roles.push(AssetRole::Unknown);
    }

    roles
}

pub fn classify_technology_tags(technologies: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();
    for technology in technologies {
        let lowered = technology.to_ascii_lowercase();
        for (needle, label, confidence) in [
            ("jenkins", "jenkins", 0.95),
            ("grafana", "grafana", 0.95),
            ("kibana", "kibana", 0.95),
            ("elasticsearch", "elasticsearch", 0.95),
            ("prometheus", "prometheus", 0.95),
            ("kubernetes", "kubernetes", 0.95),
            ("docker", "docker", 0.9),
            ("wordpress", "wordpress", 0.95),
            ("joomla", "joomla", 0.95),
            ("drupal", "drupal", 0.95),
            ("next.js", "nextjs", 0.9),
            ("express", "express", 0.9),
            ("spring", "spring", 0.9),
            ("laravel", "laravel", 0.9),
            ("django", "django", 0.9),
            ("rails", "rails", 0.9),
        ] {
            if lowered.contains(needle) {
                push_tag(
                    &mut tags,
                    SemanticTag {
                        tag: label.to_string(),
                        category: "technology".to_string(),
                        confidence,
                        evidence: vec![format!(
                            "Recognized high-interest technology '{technology}'"
                        )],
                    },
                );
            }
        }
    }
    tags
}

pub fn classify_endpoint_intents(url_or_path: &str) -> Vec<SemanticTag> {
    let path = normalize_path(url_or_path);
    let lowered = path.to_ascii_lowercase();
    let mut tags = Vec::new();

    for (needle, tag, confidence) in [
        ("/admin", "admin_surface", 0.95),
        ("/internal", "internal_surface", 0.95),
        ("/api", "api_surface", 0.8),
        ("/graphql", "graphql_surface", 0.95),
        ("/swagger", "api_documentation", 0.95),
        ("/swagger.json", "api_documentation", 0.98),
        ("/openapi", "api_documentation", 0.95),
        ("/openapi.json", "api_documentation", 0.98),
        ("/api-docs", "api_documentation", 0.96),
        ("/v2/api-docs", "api_documentation", 0.97),
        ("/v3/api-docs", "api_documentation", 0.97),
        ("/redoc", "api_documentation", 0.95),
        ("/auth", "auth_surface", 0.88),
        ("/login", "auth_surface", 0.92),
        ("/oauth", "auth_surface", 0.94),
        ("/sso", "auth_surface", 0.94),
        ("/debug", "debug_surface", 0.9),
        ("/export", "sensitive_data_operation", 0.9),
        ("/backup", "sensitive_data_operation", 0.95),
        ("/upload", "upload_surface", 0.85),
        ("/users", "user_management", 0.75),
        ("/billing", "billing_surface", 0.8),
        ("/payments", "payment_surface", 0.85),
        ("/beta", "feature_flag_route", 0.75),
        ("/feature", "feature_flag_route", 0.7),
    ] {
        if lowered.contains(needle) {
            push_tag(
                &mut tags,
                SemanticTag {
                    tag: tag.to_string(),
                    category: "endpoint_intent".to_string(),
                    confidence,
                    evidence: vec![format!("Matched path segment '{needle}' in '{path}'")],
                },
            );
        }
    }

    tags
}

pub fn classify_api_family_tags(paths: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for path in paths {
        let lowered = normalize_path(path).to_ascii_lowercase();
        let family = if lowered.starts_with("/api/") {
            lowered
                .trim_start_matches("/api/")
                .split('/')
                .next()
                .unwrap_or("root")
        } else {
            lowered
                .trim_start_matches('/')
                .split('/')
                .next()
                .unwrap_or("root")
        };
        if family.is_empty() {
            continue;
        }

        push_tag(
            &mut tags,
            SemanticTag {
                tag: format!("api_family:{family}"),
                category: "api_family".to_string(),
                confidence: if family == "graphql" { 0.95 } else { 0.72 },
                evidence: vec![format!(
                    "Classified path '{path}' into API family '{family}'"
                )],
            },
        );
    }

    tags
}

pub fn classify_auth_surface_tags(texts: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for text in texts {
        let lowered = text.to_ascii_lowercase();
        for (needle, tag, confidence) in [
            ("authorization", "auth_header_indicator", 0.9),
            ("bearer", "auth_bearer_indicator", 0.95),
            ("jwt", "auth_jwt_indicator", 0.95),
            ("oauth", "auth_oauth_indicator", 0.95),
            ("oidc", "auth_oidc_indicator", 0.95),
            ("saml", "auth_saml_indicator", 0.95),
            ("session", "auth_session_indicator", 0.8),
            ("csrf", "auth_csrf_indicator", 0.85),
            ("refresh_token", "auth_refresh_indicator", 0.9),
            ("x-api-key", "auth_api_key_indicator", 0.9),
            ("token", "auth_token_indicator", 0.8),
            ("login", "auth_surface", 0.85),
            ("auth", "auth_surface", 0.8),
        ] {
            if lowered.contains(needle) {
                push_tag(
                    &mut tags,
                    SemanticTag {
                        tag: tag.to_string(),
                        category: "auth_surface".to_string(),
                        confidence,
                        evidence: vec![format!("Matched auth indicator '{needle}' in '{text}'")],
                    },
                );
            }
        }
    }

    tags
}

pub fn classify_object_sensitivity(object_names: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for object_name in object_names {
        let lowered = object_name.to_ascii_lowercase();
        let sensitivity = if contains_any(
            &lowered,
            &[
                "user",
                "account",
                "organization",
                "billing",
                "payment",
                "invoice",
                "admin",
                "token",
                "secret",
            ],
        ) {
            Some(("object_sensitivity:high", 0.92))
        } else if contains_any(
            &lowered,
            &["customer", "project", "report", "file", "document"],
        ) {
            Some(("object_sensitivity:medium", 0.75))
        } else {
            Some(("object_sensitivity:low", 0.55))
        };

        if let Some((tag, confidence)) = sensitivity {
            push_tag(
                &mut tags,
                SemanticTag {
                    tag: tag.to_string(),
                    category: "object_sensitivity".to_string(),
                    confidence,
                    evidence: vec![format!("Inferred object '{}' as {tag}", object_name)],
                },
            );
        }
    }

    tags
}

pub fn classify_js_feature_flag_tags(flags: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for flag in flags {
        let lowered = flag.to_ascii_lowercase();
        for (needle, tag, confidence) in [
            ("feature", "feature_flag", 0.8),
            ("beta", "feature_flag", 0.86),
            ("experimental", "feature_flag", 0.86),
            ("canary", "feature_flag", 0.82),
            ("debug", "debug_feature", 0.9),
            ("admin", "privileged_feature", 0.88),
            ("internal", "internal_feature", 0.9),
        ] {
            if lowered.contains(needle) {
                push_tag(
                    &mut tags,
                    SemanticTag {
                        tag: tag.to_string(),
                        category: "js_feature_flag".to_string(),
                        confidence,
                        evidence: vec![format!(
                            "Matched JS feature flag hint '{needle}' in '{flag}'"
                        )],
                    },
                );
            }
        }
    }

    tags
}

pub fn classify_parameter_intents(parameters: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();

    for parameter in parameters {
        let lowered = parameter.to_ascii_lowercase();
        let candidates =
            if contains_any(&lowered, &["user_id", "account_id", "org_id"]) || lowered == "id" {
                vec![("identifier", 0.8)]
            } else if contains_any(&lowered, &["redirect", "next", "return", "url"]) {
                vec![("redirect_control", 0.9)]
            } else if contains_any(&lowered, &["file", "path", "template"]) {
                vec![("file_or_path_control", 0.85)]
            } else if contains_any(&lowered, &["q", "search"]) {
                vec![("search_input", 0.6)]
            } else if contains_any(&lowered, &["token", "key", "secret"]) {
                vec![("secret_or_token", 0.95)]
            } else if lowered.contains("debug") {
                vec![("debug_control", 0.9)]
            } else {
                Vec::new()
            };

        for (tag, confidence) in candidates {
            push_tag(
                &mut tags,
                SemanticTag {
                    tag: tag.to_string(),
                    category: "parameter_intent".to_string(),
                    confidence,
                    evidence: vec![format!("Matched parameter intent in '{parameter}'")],
                },
            );
        }
    }

    tags
}

fn environment_from_tag(tag: &str) -> Option<EnvironmentType> {
    match tag {
        "production" => Some(EnvironmentType::Production),
        "staging" => Some(EnvironmentType::Staging),
        "development" => Some(EnvironmentType::Development),
        "testing" => Some(EnvironmentType::Testing),
        "internal" => Some(EnvironmentType::Internal),
        "legacy" => Some(EnvironmentType::Legacy),
        "unknown" => Some(EnvironmentType::Unknown),
        _ => None,
    }
}

fn role_from_tag(tag: &str) -> Option<AssetRole> {
    match tag {
        "authentication" => Some(AssetRole::Authentication),
        "admin_dashboard" => Some(AssetRole::AdminDashboard),
        "monitoring" => Some(AssetRole::Monitoring),
        "logging" => Some(AssetRole::Logging),
        "cicd" => Some(AssetRole::CICD),
        "api_gateway" => Some(AssetRole::ApiGateway),
        "storage" => Some(AssetRole::Storage),
        "analytics" => Some(AssetRole::Analytics),
        "documentation" => Some(AssetRole::Documentation),
        "customer_app" => Some(AssetRole::CustomerApp),
        "unknown" => Some(AssetRole::Unknown),
        _ => None,
    }
}

fn normalize_path(url_or_path: &str) -> String {
    if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
        if let Ok(parsed) = Url::parse(url_or_path) {
            return parsed.path().to_string();
        }
    }
    url_or_path.to_string()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn push_tag(tags: &mut Vec<SemanticTag>, tag: SemanticTag) {
    let key = (tag.tag.clone(), tag.category.clone());
    let mut index_by_key = BTreeMap::new();
    for (index, existing) in tags.iter().enumerate() {
        index_by_key.insert((existing.tag.clone(), existing.category.clone()), index);
    }

    if let Some(index) = index_by_key.get(&key).copied() {
        let existing = &mut tags[index];
        existing.confidence = existing.confidence.max(tag.confidence);
        merge_strings(&mut existing.evidence, tag.evidence);
    } else {
        tags.push(tag);
    }

    tags.sort_by(|left, right| {
        (left.category.as_str(), left.tag.as_str())
            .cmp(&(right.category.as_str(), right.tag.as_str()))
    });
}

fn merge_strings(target: &mut Vec<String>, values: Vec<String>) {
    let mut seen = target.iter().cloned().collect::<BTreeSet<_>>();
    for value in values {
        if seen.insert(value.clone()) {
            target.push(value);
        }
    }
    target.sort();
}

#[cfg(test)]
mod tests {
    use crate::models::{AssetRole, EnvironmentType};

    use super::{
        classify_api_family_tags, classify_auth_surface_tags, classify_endpoint_intents,
        classify_environments, classify_js_feature_flag_tags, classify_object_sensitivity,
        classify_parameter_intents, classify_roles,
    };

    #[test]
    fn environment_classification_matches_keywords() {
        let values = classify_environments(&[
            "staging-auth.example.com".to_string(),
            "corp-internal".to_string(),
        ]);
        assert!(values.contains(&EnvironmentType::Staging));
        assert!(values.contains(&EnvironmentType::Internal));
    }

    #[test]
    fn role_classification_matches_keywords() {
        let values = classify_roles(
            &["jenkins-admin-console".to_string(), "/login".to_string()],
            &["Jenkins".to_string()],
        );
        assert!(values.contains(&AssetRole::CICD));
        assert!(values.contains(&AssetRole::AdminDashboard));
        assert!(values.contains(&AssetRole::Authentication));
    }

    #[test]
    fn endpoint_intent_classification_matches_paths() {
        let tags = classify_endpoint_intents("https://app.example.com/swagger/openapi");
        assert!(tags.iter().any(|tag| tag.tag == "api_documentation"));
    }

    #[test]
    fn parameter_intent_classification_matches_parameters() {
        let tags = classify_parameter_intents(&[
            "returnUrl".to_string(),
            "token".to_string(),
            "user_id".to_string(),
        ]);
        assert!(tags.iter().any(|tag| tag.tag == "redirect_control"));
        assert!(tags.iter().any(|tag| tag.tag == "secret_or_token"));
        assert!(tags.iter().any(|tag| tag.tag == "identifier"));
    }

    #[test]
    fn api_family_and_auth_surface_classification_match_expected_inputs() {
        let families =
            classify_api_family_tags(&["/api/v1/users".to_string(), "/graphql".to_string()]);
        assert!(families
            .iter()
            .any(|tag| tag.tag.starts_with("api_family:")));

        let auth = classify_auth_surface_tags(&[
            "Authorization: Bearer jwt".to_string(),
            "/oauth/login".to_string(),
        ]);
        assert!(auth.iter().any(|tag| tag.tag == "auth_bearer_indicator"));
        assert!(auth.iter().any(|tag| tag.tag == "auth_surface"));
    }

    #[test]
    fn object_sensitivity_and_feature_flags_classify_candidates() {
        let object_tags = classify_object_sensitivity(&["User".to_string(), "Billing".to_string()]);
        assert!(object_tags
            .iter()
            .any(|tag| tag.tag == "object_sensitivity:high"));

        let feature_tags =
            classify_js_feature_flag_tags(&["adminOnlyBeta".to_string(), "debugMode".to_string()]);
        assert!(feature_tags.iter().any(|tag| tag.tag == "feature_flag"));
        assert!(feature_tags.iter().any(|tag| tag.tag == "debug_feature"));
    }
}
