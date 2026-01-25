use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MetricsContext {
    scenario: Arc<str>,
    scenario_tags: Arc<[(String, String)]>,
}

impl MetricsContext {
    #[must_use]
    pub fn new(scenario: Arc<str>, scenario_tags: Arc<[(String, String)]>) -> Self {
        Self {
            scenario,
            scenario_tags,
        }
    }

    #[must_use]
    pub fn scenario(&self) -> &str {
        self.scenario.as_ref()
    }

    #[must_use]
    pub fn scenario_arc(&self) -> Arc<str> {
        self.scenario.clone()
    }

    #[must_use]
    pub fn scenario_tags(&self) -> &[(String, String)] {
        self.scenario_tags.as_ref()
    }

    /// Merge scenario-level tags into an existing tag vec.
    ///
    /// - Does not override already-present keys.
    /// - Skips any keys in `reserved_keys`.
    pub fn merge_scenario_tags_if_missing(
        &self,
        tags: &mut Vec<(String, String)>,
        reserved_keys: &[&str],
    ) {
        for (k, v) in self.scenario_tags.iter() {
            if reserved_keys.contains(&k.as_str()) {
                continue;
            }
            if tags.iter().any(|(ek, _)| ek == k) {
                continue;
            }
            tags.push((k.clone(), v.clone()));
        }
    }

    /// Merge the full base tag set used for most metrics: `scenario` + scenario-level tags.
    ///
    /// - Adds `scenario` only if missing.
    /// - Does not override already-present keys.
    /// - Skips any keys in `reserved_keys`.
    pub fn merge_base_tags_if_missing(
        &self,
        tags: &mut Vec<(String, String)>,
        reserved_keys: &[&str],
    ) {
        if !reserved_keys.contains(&"scenario") && !tags.iter().any(|(k, _)| k == "scenario") {
            tags.push(("scenario".to_string(), self.scenario.to_string()));
        }

        self.merge_scenario_tags_if_missing(tags, reserved_keys);
    }

    /// Returns references to scenario-level tags (no `scenario`), suitable for passing to
    /// `*_metrics.record_*` as extra tags.
    #[must_use]
    pub fn scenario_tag_refs(&self, reserved_keys: &[&str]) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        for (k, v) in self.scenario_tags.iter() {
            if reserved_keys.contains(&k.as_str()) {
                continue;
            }
            out.push((k.as_str(), v.as_str()));
        }
        out
    }
}
