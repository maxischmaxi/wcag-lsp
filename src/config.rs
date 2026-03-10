use crate::rules::{Severity, WcagLevel};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct RawConfig {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
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
    pub severity_a: Option<Severity>,
    pub severity_aa: Option<Severity>,
    pub severity_aaa: Option<Severity>,
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
            severity_a: Some(Severity::Error),
            severity_aa: Some(Severity::Warning),
            severity_aaa: Some(Severity::Warning),
            rule_overrides: HashMap::new(),
            ignore_patterns: vec![],
        }
    }
}

impl Config {
    pub fn from_file(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => Self::parse_json(&content),
            Some("toml") => Self::parse(&content),
            _ => Self::default(),
        }
    }

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
        /// Parses a severity string into an `Option<Option<Severity>>`:
        /// - `Some(None)` means explicitly disabled ("off")
        /// - `Some(Some(severity))` means a valid severity
        /// - `None` means unrecognized value (use default)
        fn parse_level_severity(s: &str) -> Option<Option<Severity>> {
            match s.to_lowercase().as_str() {
                "error" => Some(Some(Severity::Error)),
                "warning" | "warn" => Some(Some(Severity::Warning)),
                "off" | "false" | "disable" => Some(None),
                _ => None,
            }
        }

        let severity_a = raw
            .severity
            .get("A")
            .and_then(|s| parse_level_severity(s))
            .unwrap_or(Some(Severity::Error));
        let severity_aa = raw
            .severity
            .get("AA")
            .and_then(|s| parse_level_severity(s))
            .unwrap_or(Some(Severity::Warning));
        let severity_aaa = raw
            .severity
            .get("AAA")
            .and_then(|s| parse_level_severity(s))
            .unwrap_or(Some(Severity::Warning));

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

    pub fn severity_for_level(&self, level: WcagLevel) -> Option<Severity> {
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

    /// Returns the effective severity for a rule, or `None` if the rule is disabled
    /// (either by per-rule override or by level being "off").
    /// A per-rule severity override takes precedence over a disabled level.
    pub fn effective_severity(&self, rule_id: &str, level: WcagLevel) -> Option<Severity> {
        match self.rule_overrides.get(rule_id) {
            Some(RuleOverride::Off) => None,
            Some(RuleOverride::Severity(s)) => Some(*s),
            None => self.severity_for_level(level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.severity_a, Some(Severity::Error));
        assert_eq!(config.severity_aa, Some(Severity::Warning));
        assert_eq!(config.severity_aaa, Some(Severity::Warning));
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
        assert_eq!(config.severity_aa, Some(Severity::Error));
        assert_eq!(
            config.rule_overrides.get("heading-order"),
            Some(&RuleOverride::Off)
        );
        assert!(!config.is_rule_enabled("heading-order"));
        assert!(config.is_rule_enabled("img-alt"));
        assert_eq!(
            config.effective_severity("img-alt", WcagLevel::A),
            Some(Severity::Warning)
        );
        assert_eq!(config.ignore_patterns.len(), 2);
    }

    #[test]
    fn test_invalid_toml_returns_defaults() {
        let config = Config::parse("this is not valid toml {{{}}}");
        assert_eq!(config.severity_a, Some(Severity::Error));
    }

    #[test]
    fn test_empty_config_returns_defaults() {
        let config = Config::parse("");
        assert_eq!(config.severity_a, Some(Severity::Error));
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
        assert_eq!(config.severity_aa, Some(Severity::Error));
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
        assert_eq!(config.severity_a, Some(Severity::Error));
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
        assert_eq!(config.severity_aa, Some(Severity::Error));
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
        assert_eq!(config.severity_aa, Some(Severity::Error));
    }

    #[test]
    fn test_from_dir_no_config_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::from_dir(dir.path());
        assert_eq!(config.severity_a, Some(Severity::Error));
        assert_eq!(config.severity_aa, Some(Severity::Warning));
    }

    #[test]
    fn test_from_file_toml() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("custom.toml");
        std::fs::write(&file_path, "[severity]\nAA = \"error\"\n").unwrap();
        let config = Config::from_file(&file_path);
        assert_eq!(config.severity_aa, Some(Severity::Error));
    }

    #[test]
    fn test_from_file_json() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("custom.json");
        std::fs::write(&file_path, r#"{"severity": {"AA": "error"}}"#).unwrap();
        let config = Config::from_file(&file_path);
        assert_eq!(config.severity_aa, Some(Severity::Error));
    }

    #[test]
    fn test_from_file_nonexistent_returns_defaults() {
        let config = Config::from_file(std::path::Path::new("/nonexistent/path.toml"));
        assert_eq!(config.severity_a, Some(Severity::Error));
        assert_eq!(config.severity_aa, Some(Severity::Warning));
    }

    #[test]
    fn test_from_file_unknown_extension_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yaml");
        std::fs::write(&file_path, "severity:\n  AA: error\n").unwrap();
        let config = Config::from_file(&file_path);
        assert_eq!(config.severity_aa, Some(Severity::Warning));
    }

    #[test]
    fn test_level_off_toml() {
        let config = Config::parse(
            r#"
[severity]
A = "off"
AA = "error"
AAA = "disable"
"#,
        );
        assert_eq!(config.severity_a, None);
        assert_eq!(config.severity_aa, Some(Severity::Error));
        assert_eq!(config.severity_aaa, None);
    }

    #[test]
    fn test_level_off_json() {
        let config =
            Config::parse_json(r#"{"severity": {"A": "off", "AA": "false", "AAA": "disable"}}"#);
        assert_eq!(config.severity_a, None);
        assert_eq!(config.severity_aa, None);
        assert_eq!(config.severity_aaa, None);
    }

    #[test]
    fn test_level_off_disables_rules() {
        let config = Config::parse(
            r#"
[severity]
A = "off"
"#,
        );
        assert_eq!(config.effective_severity("img-alt", WcagLevel::A), None);
        assert_eq!(
            config.effective_severity("some-aa-rule", WcagLevel::AA),
            Some(Severity::Warning)
        );
    }

    #[test]
    fn test_per_rule_override_takes_precedence_over_level_off() {
        let config = Config::parse(
            r#"
[severity]
A = "off"

[rules]
img-alt = "error"
"#,
        );
        // Level A is off, but img-alt has an explicit override
        assert_eq!(
            config.effective_severity("img-alt", WcagLevel::A),
            Some(Severity::Error)
        );
        // Other Level A rules are disabled
        assert_eq!(
            config.effective_severity("other-a-rule", WcagLevel::A),
            None
        );
    }

    #[test]
    fn test_json_with_schema_field() {
        let config = Config::parse_json(
            r#"{
                "$schema": "https://raw.githubusercontent.com/maxischmaxi/wcag-lsp/main/wcag-lsp.schema.json",
                "severity": { "A": "error", "AA": "warning" },
                "rules": { "img-alt": "warning" }
            }"#,
        );
        assert_eq!(config.severity_a, Some(Severity::Error));
        assert_eq!(config.severity_aa, Some(Severity::Warning));
        assert_eq!(
            config.effective_severity("img-alt", WcagLevel::A),
            Some(Severity::Warning)
        );
    }
}
