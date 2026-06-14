use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use aisix_utils::jsonschema::format_evaluation_error;
use aisix_guardrail::guardrails::{
    configs::{BedrockGuardrailConfig, RegexGuardrailConfig},
    identifiers,
};

static SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("guardrails-schema.json"))
        .expect("Invalid JSON document for Guardrail schema")
});
pub static SCHEMA_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    jsonschema::validator_for(&SCHEMA).expect("Invalid JSON schema for Guardrail")
});

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "config")]
pub enum GuardrailConfig {
    #[serde(rename = "bedrock")]
    Bedrock(BedrockGuardrailConfig),

    #[serde(rename = "regex")]
    Regex(RegexGuardrailConfig),
}

impl GuardrailConfig {
    pub fn guardrail_type(&self) -> &'static str {
        match self {
            Self::Bedrock(_) => identifiers::BEDROCK,
            Self::Regex(_) => identifiers::REGEX,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Guardrail {
    pub name: String,

    #[serde(flatten)]
    #[schema(inline)]
    pub guardrail: GuardrailConfig,
}

impl Guardrail {
    pub fn guardrail_type(&self) -> &'static str {
        self.guardrail.guardrail_type()
    }
}

pub fn validate_guardrail_definition(key: &str, value: &Guardrail) -> Result<(), String> {
    let evaluation = SCHEMA_VALIDATOR.evaluate(
        &serde_json::to_value(value)
            .map_err(|error| format!("Failed to serialize guardrail for validation: {}", error))?,
    );
    if !evaluation.flag().valid {
        return Err(format!(
            r#"JSON schema validation error on guardrail "{key}": {}"#,
            format_evaluation_error(&evaluation)
        ));
    }

    validate_config(key, &value.guardrail)?;

    Ok(())
}

fn validate_config(_key: &str, config: &GuardrailConfig) -> Result<(), String> {
    match config {
        GuardrailConfig::Bedrock(_) => Ok(()),
        GuardrailConfig::Regex(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::{Guardrail, GuardrailConfig, SCHEMA, SCHEMA_VALIDATOR, format_evaluation_error};

    #[test]
    fn test_valid_jsonschema() {
        assert!(jsonschema::meta::is_valid(&SCHEMA));
    }

    #[rstest::rstest]
    #[case::bedrock_ok(json!({
        "name": "bedrock-prod",
        "type": "bedrock",
        "config": {
            "identifier": "guardrail-123",
            "version": "1",
            "region": "us-east-1",
            "access_key_id": "AKIA123",
            "secret_access_key": "secret"
        }
    }), true, None)]
    #[case::regex_ok(json!({
        "name": "regex-prod",
        "type": "regex",
        "config": {
            "pattern": "secret",
            "block_reason": "matched blocked content"
        }
    }), true, None)]
    #[case::missing_type(json!({
        "name": "bedrock-prod",
        "config": {
            "identifier": "guardrail-123",
            "version": "1",
            "region": "us-east-1",
            "access_key_id": "AKIA123",
            "secret_access_key": "secret"
        }
    }), false, Some(r#"property "/" validation failed: "type" is a required property"#.to_string()))]
    #[case::missing_identifier(json!({
        "name": "bedrock-prod",
        "type": "bedrock",
        "config": {
            "version": "1",
            "region": "us-east-1",
            "access_key_id": "AKIA123",
            "secret_access_key": "secret"
        }
    }), false, Some(r#"property "/config" validation failed: "identifier" is a required property"#.to_string()))]
    #[case::missing_version(json!({
        "name": "bedrock-prod",
        "type": "bedrock",
        "config": {
            "identifier": "guardrail-123",
            "region": "us-east-1",
            "access_key_id": "AKIA123",
            "secret_access_key": "secret"
        }
    }), false, Some(r#"property "/config" validation failed: "version" is a required property"#.to_string()))]
    #[case::missing_region(json!({
        "name": "bedrock-prod",
        "type": "bedrock",
        "config": {
            "identifier": "guardrail-123",
            "version": "1",
            "access_key_id": "AKIA123",
            "secret_access_key": "secret"
        }
    }), false, Some(r#"property "/config" validation failed: "region" is a required property"#.to_string()))]
    #[case::regex_missing_pattern(json!({
        "name": "regex-prod",
        "type": "regex",
        "config": {
            "block_reason": "matched blocked content"
        }
    }), false, Some(r#"property "/config" validation failed: "pattern" is a required property"#.to_string()))]
    #[case::invalid_root_additional_property(json!({
        "name": "bedrock-prod",
        "type": "bedrock",
        "config": {
            "identifier": "guardrail-123",
            "version": "1",
            "region": "us-east-1",
            "access_key_id": "AKIA123",
            "secret_access_key": "secret"
        },
        "extra": "not allowed"
    }), false, Some(r#"property "/" validation failed: Additional properties are not allowed ('extra' was unexpected)"#.to_string()))]
    fn schemas(
        #[case] input: serde_json::Value,
        #[case] ok: bool,
        #[case] expected_error: Option<String>,
    ) {
        let evaluation = SCHEMA_VALIDATOR.evaluate(&input);

        assert_eq!(evaluation.flag().valid, ok, "unexpected evaluation result");
        if !ok {
            assert_eq!(
                format_evaluation_error(&evaluation),
                expected_error.unwrap(),
                "unexpected error message"
            );
        }
    }

    #[test]
    fn deserialize_regex_guardrail_rejects_invalid_patterns() {
        let error = serde_json::from_value::<Guardrail>(json!({
            "name": "regex-invalid",
            "type": "regex",
            "config": {
                "pattern": "[",
                "block_reason": "matched blocked content"
            }
        }))
        .expect_err("invalid regex pattern should be rejected while loading config");

        assert!(
            error
                .to_string()
                .contains("invalid regex guardrail pattern")
        );
    }

    #[test]
    fn deserialize_guardrail_preserves_type_information() {
        let guardrail: Guardrail = serde_json::from_value(json!({
            "name": "bedrock-prod",
            "type": "bedrock",
            "config": {
                "identifier": "guardrail-123",
                "version": "1",
                "region": "us-east-1",
                "access_key_id": "AKIA123",
                "secret_access_key": "secret"
            }
        }))
        .unwrap();

        assert_eq!(guardrail.name, "bedrock-prod");
        assert_eq!(guardrail.guardrail_type(), "bedrock");
        assert_matches!(guardrail.guardrail, GuardrailConfig::Bedrock(_));
    }

    #[test]
    fn deserialize_regex_guardrail_preserves_type_information() {
        let guardrail: Guardrail = serde_json::from_value(json!({
            "name": "regex-prod",
            "type": "regex",
            "config": {
                "pattern": "secret",
                "block_reason": "matched blocked content"
            }
        }))
        .unwrap();

        assert_eq!(guardrail.name, "regex-prod");
        assert_eq!(guardrail.guardrail_type(), "regex");
        assert_matches!(guardrail.guardrail, GuardrailConfig::Regex(_));
    }
}
