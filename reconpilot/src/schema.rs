use std::collections::BTreeSet;

use serde_json::Value;

use crate::{
    auth,
    models::{ApiSchema, GraphQlObservation},
};

pub fn parse_schema_artifact(location: &str, content: &str) -> Option<ApiSchema> {
    let lowered_location = location.to_ascii_lowercase();
    let lowered_content = content.to_ascii_lowercase();

    if let Ok(value) = serde_json::from_str::<Value>(content) {
        if let Some(schema) = parse_json_schema(location, &value) {
            return Some(schema);
        }
    }

    if lowered_location.contains("swagger")
        || lowered_location.contains("openapi")
        || lowered_location.contains("api-docs")
        || lowered_content.contains("redoc")
    {
        return Some(ApiSchema {
            schema_type: if lowered_content.contains("redoc") {
                "redoc_reference".to_string()
            } else {
                "schema_reference".to_string()
            },
            schema_location: location.to_string(),
            detected_version: None,
            endpoints: Vec::new(),
            auth_methods: auth::detect_auth_indicators(content),
            objects: Vec::new(),
        });
    }

    None
}

pub fn detect_graphql_observations(location: &str, content: &str) -> Vec<GraphQlObservation> {
    let lowered_location = location.to_ascii_lowercase();
    let lowered_content = content.to_ascii_lowercase();

    let mut indicators = BTreeSet::new();
    for needle in [
        "/graphql",
        "apollo",
        "graphql playground",
        "graphiql",
        "__schema",
        "introspectionquery",
        "type query",
    ] {
        if lowered_location.contains(needle) || lowered_content.contains(needle) {
            indicators.insert(needle.to_string());
        }
    }

    if indicators.is_empty() {
        return Vec::new();
    }

    let endpoint = if lowered_location.contains("/graphql") {
        location.to_string()
    } else {
        "/graphql".to_string()
    };

    vec![GraphQlObservation {
        endpoint,
        introspection_detected: lowered_content.contains("__schema")
            || lowered_content.contains("introspectionquery"),
        schema_indicators: indicators.into_iter().collect(),
        auth_indicators: auth::detect_auth_indicators(content),
        notes: vec![
            "GraphQL indicators were inferred from local artifacts only.".to_string(),
            "No GraphQL query or introspection request was executed.".to_string(),
        ],
    }]
}

fn parse_json_schema(location: &str, value: &Value) -> Option<ApiSchema> {
    let schema_type = if value.get("openapi").is_some() {
        Some("openapi")
    } else if value.get("swagger").is_some() {
        Some("swagger")
    } else if value.get("paths").is_some() {
        Some("api_schema")
    } else {
        None
    }?;

    let detected_version = value
        .get("openapi")
        .and_then(Value::as_str)
        .or_else(|| value.get("swagger").and_then(Value::as_str))
        .map(ToOwned::to_owned);

    let paths = value
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut endpoints = Vec::new();
    let mut objects = BTreeSet::new();
    let mut auth_methods = BTreeSet::new();

    for (path, path_value) in paths {
        if let Some(methods) = path_value.as_object() {
            for (method, method_value) in methods {
                if !is_http_method(method) {
                    continue;
                }
                endpoints.push(format!("{} {}", method.to_ascii_uppercase(), path));

                collect_schema_object_refs(method_value, &mut objects);
                collect_auth_methods(method_value, &mut auth_methods);

                for parameter in method_value
                    .get("parameters")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    collect_schema_object_refs(parameter, &mut objects);
                }
            }
        }
    }

    if let Some(components) = value.get("components") {
        collect_schema_object_refs(components, &mut objects);
        collect_auth_methods(components, &mut auth_methods);
    }
    if let Some(definitions) = value.get("definitions") {
        collect_schema_object_refs(definitions, &mut objects);
    }
    if let Some(security_definitions) = value.get("securityDefinitions") {
        collect_auth_methods(security_definitions, &mut auth_methods);
    }

    endpoints.sort();
    let mut objects = objects.into_iter().collect::<Vec<_>>();
    objects.sort();
    let mut auth_methods = auth_methods.into_iter().collect::<Vec<_>>();
    auth_methods.sort();

    Some(ApiSchema {
        schema_type: schema_type.to_string(),
        schema_location: location.to_string(),
        detected_version,
        endpoints,
        auth_methods,
        objects,
    })
}

fn collect_schema_object_refs(value: &Value, objects: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map {
                if key == "$ref" {
                    if let Some(reference) = entry.as_str() {
                        if let Some(object_name) = reference.rsplit('/').next() {
                            objects.insert(object_name.to_string());
                        }
                    }
                    continue;
                }
                if matches!(key.as_str(), "schemas" | "definitions") {
                    if let Some(object) = entry.as_object() {
                        for name in object.keys() {
                            objects.insert(name.to_string());
                        }
                    }
                }
                collect_schema_object_refs(entry, objects);
            }
        }
        Value::Array(array) => {
            for entry in array {
                collect_schema_object_refs(entry, objects);
            }
        }
        _ => {}
    }
}

fn collect_auth_methods(value: &Value, methods: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map {
                if key.eq_ignore_ascii_case("security")
                    || key.eq_ignore_ascii_case("securityschemes")
                {
                    if let Some(object) = entry.as_object() {
                        for (scheme_name, scheme_value) in object {
                            methods.insert(scheme_name.to_string());
                            if let Some(value_type) =
                                scheme_value.get("type").and_then(Value::as_str)
                            {
                                methods.insert(value_type.to_string());
                            }
                        }
                    } else if let Some(array) = entry.as_array() {
                        for item in array {
                            if let Some(object) = item.as_object() {
                                for name in object.keys() {
                                    methods.insert(name.to_string());
                                }
                            }
                        }
                    }
                }

                if let Some(value_type) = entry
                    .get("scheme")
                    .and_then(Value::as_str)
                    .or_else(|| entry.get("type").and_then(Value::as_str))
                {
                    let lowered = value_type.to_ascii_lowercase();
                    if matches!(
                        lowered.as_str(),
                        "oauth2" | "http" | "bearer" | "apikey" | "apiKey" | "openidconnect"
                    ) {
                        methods.insert(value_type.to_string());
                    }
                }

                collect_auth_methods(entry, methods);
            }
        }
        Value::Array(array) => {
            for entry in array {
                collect_auth_methods(entry, methods);
            }
        }
        _ => {}
    }
}

fn is_http_method(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
    )
}

#[cfg(test)]
mod tests {
    use super::{detect_graphql_observations, parse_schema_artifact};

    #[test]
    fn swagger_openapi_detection_and_schema_parsing_work() {
        let schema = parse_schema_artifact(
            "swagger.json",
            r##"{
              "openapi": "3.0.2",
              "paths": {
                "/users/{id}": {
                  "get": {
                    "parameters": [{"name": "id", "in": "path"}],
                    "responses": {
                      "200": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/User"}}}}
                    }
                  }
                }
              },
              "components": {
                "securitySchemes": {
                  "bearerAuth": {"type": "http", "scheme": "bearer"}
                },
                "schemas": {
                  "User": {"type": "object"}
                }
              }
            }"##,
        )
        .expect("schema should be parsed");
        assert_eq!(schema.schema_type, "openapi");
        assert_eq!(schema.detected_version.as_deref(), Some("3.0.2"));
        assert!(schema
            .endpoints
            .iter()
            .any(|value| value == "GET /users/{id}"));
        assert!(schema.objects.iter().any(|value| value == "User"));
        assert!(schema
            .auth_methods
            .iter()
            .any(|value| value == "bearerAuth"));
    }

    #[test]
    fn graphql_detection_finds_local_indicators() {
        let observations = detect_graphql_observations(
            "bundle.js",
            "const endpoint = '/graphql'; const query = '__schema { queryType { name } }'; ApolloClient();",
        );
        assert_eq!(observations.len(), 1);
        assert!(observations[0].introspection_detected);
        assert!(observations[0]
            .schema_indicators
            .iter()
            .any(|value| value.contains("apollo")));
    }
}
