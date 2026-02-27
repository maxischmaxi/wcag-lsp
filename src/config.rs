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
    pub fn from_file(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> Self {
        let raw: RawConfig = match toml::from_str(content) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };

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
        let config = Config::from_str(
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
        let config = Config::from_str("this is not valid toml {{{}}}");
        assert_eq!(config.severity_a, Severity::Error);
    }

    #[test]
    fn test_empty_config_returns_defaults() {
        let config = Config::from_str("");
        assert_eq!(config.severity_a, Severity::Error);
        assert!(config.rule_overrides.is_empty());
    }
}
