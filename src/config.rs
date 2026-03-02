use crate::rules::{Severity, WcagLevel};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct RawConfig {
    #[serde(default)]
    pub severity: HashMap<String, String>,
    #[serde(default)]
    pub rules: HashMap<String, String>,
    #[serde(default)]
    pub ignore: IgnoreConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug)]
pub struct Config {
    pub severity_a: Severity,
    pub severity_aa: Severity,
    pub severity_aaa: Severity,
    pub rule_overrides: HashMap<String, RuleOverride>,
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuleOverride {
    Off,
    Severity(Severity),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            severity_a: Severity::Error,
            severity_aa: Severity::Warning,
            severity_aaa: Severity::Warning,
            rule_overrides: HashMap::new(),
            ignore_patterns: vec![],
        }
    }
}

impl Config {
    pub fn from_dir(dir: &Path) -> Self {
        let toml_path = dir.join(".wcag.toml");
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            return Self::parse(&content);
        }

        let json_path = dir.join(".wcag.json");
        if let Ok(content) = std::fs::read_to_string(&json_path) {
            return Self::parse_json(&content);
        }

        Self::default()
    }

    pub fn parse_json(content: &str) -> Self {
        let raw: RawConfig = match serde_json::from_str(content) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };
        Self::from_raw(raw)
    }

    pub fn parse(content: &str) -> Self {
        let raw: RawConfig = match toml::from_str(content) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawConfig) -> Self {
        let parse_severity = |s: &str| -> Option<Severity> {
            match s.to_lowercase().as_str() {
                "error" => Some(Severity::Error),
                "warning" | "warn" => Some(Severity::Warning),
                _ => None,
            }
        };

        let severity_a = raw
            .severity
            .get("A")
            .and_then(|s| parse_severity(s))
            .unwrap_or(Severity::Error);
        let severity_aa = raw
            .severity
            .get("AA")
            .and_then(|s| parse_severity(s))
            .unwrap_or(Severity::Warning);
        let severity_aaa = raw
            .severity
            .get("AAA")
            .and_then(|s| parse_severity(s))
            .unwrap_or(Severity::Warning);

        let mut rule_overrides = HashMap::new();
        for (rule_id, value) in &raw.rules {
            let override_val = match value.to_lowercase().as_str() {
                "off" | "false" | "disable" => RuleOverride::Off,
                "error" => RuleOverride::Severity(Severity::Error),
                "warning" | "warn" => RuleOverride::Severity(Severity::Warning),
                _ => continue,
            };
            rule_overrides.insert(rule_id.clone(), override_val);
        }

        Config {
            severity_a,
            severity_aa,
            severity_aaa,
            rule_overrides,
            ignore_patterns: raw.ignore.patterns,
        }
    }

    pub fn severity_for_level(&self, level: WcagLevel) -> Severity {
        match level {
            WcagLevel::A => self.severity_a,
            WcagLevel::AA => self.severity_aa,
            WcagLevel::AAA => self.severity_aaa,
        }
    }

    pub fn is_rule_enabled(&self, rule_id: &str) -> bool {
        self.rule_overrides
            .get(rule_id)
            .map(|o| *o != RuleOverride::Off)
            .unwrap_or(true)
    }

    pub fn effective_severity(&self, rule_id: &str, level: WcagLevel) -> Severity {
        if let Some(RuleOverride::Severity(s)) = self.rule_overrides.get(rule_id) {
            return *s;
        }
        self.severity_for_level(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.severity_a, Severity::Error);
        assert_eq!(config.severity_aa, Severity::Warning);
        assert_eq!(config.severity_aaa, Severity::Warning);
    }

    #[test]
    fn test_parse_config() {
        let config = Config::parse(
            r#"
[severity]
A = "error"
AA = "error"
AAA = "warning"

[rules]
img-alt = "warning"
heading-order = "off"

[ignore]
patterns = ["node_modules/**", "dist/**"]
"#,
        );
        assert_eq!(config.severity_aa, Severity::Error);
        assert_eq!(
            config.rule_overrides.get("heading-order"),
            Some(&RuleOverride::Off)
        );
        assert!(!config.is_rule_enabled("heading-order"));
        assert!(config.is_rule_enabled("img-alt"));
        assert_eq!(
            config.effective_severity("img-alt", WcagLevel::A),
            Severity::Warning
        );
        assert_eq!(config.ignore_patterns.len(), 2);
    }

    #[test]
    fn test_invalid_toml_returns_defaults() {
        let config = Config::parse("this is not valid toml {{{}}}");
        assert_eq!(config.severity_a, Severity::Error);
    }

    #[test]
    fn test_empty_config_returns_defaults() {
        let config = Config::parse("");
        assert_eq!(config.severity_a, Severity::Error);
        assert!(config.rule_overrides.is_empty());
    }

    #[test]
    fn test_parse_json() {
        let config = Config::parse_json(
            r#"{
                "severity": { "A": "error", "AA": "error" },
                "rules": { "heading-order": "off", "img-alt": "warning" },
                "ignore": { "patterns": ["node_modules/**"] }
            }"#,
        );
        assert_eq!(config.severity_aa, Severity::Error);
        assert_eq!(
            config.rule_overrides.get("heading-order"),
            Some(&RuleOverride::Off)
        );
        assert_eq!(
            config.rule_overrides.get("img-alt"),
            Some(&RuleOverride::Severity(Severity::Warning))
        );
        assert_eq!(config.ignore_patterns, vec!["node_modules/**"]);
    }

    #[test]
    fn test_invalid_json_returns_defaults() {
        let config = Config::parse_json("not json {{{");
        assert_eq!(config.severity_a, Severity::Error);
        assert!(config.rule_overrides.is_empty());
    }

    #[test]
    fn test_from_dir_prefers_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".wcag.toml"),
            "[severity]\nAA = \"error\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".wcag.json"),
            r#"{"severity": {"AA": "warning"}}"#,
        )
        .unwrap();
        let config = Config::from_dir(dir.path());
        assert_eq!(config.severity_aa, Severity::Error);
    }

    #[test]
    fn test_from_dir_falls_back_to_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".wcag.json"),
            r#"{"severity": {"AA": "error"}}"#,
        )
        .unwrap();
        let config = Config::from_dir(dir.path());
        assert_eq!(config.severity_aa, Severity::Error);
    }

    #[test]
    fn test_from_dir_no_config_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::from_dir(dir.path());
        assert_eq!(config.severity_a, Severity::Error);
        assert_eq!(config.severity_aa, Severity::Warning);
    }
}
