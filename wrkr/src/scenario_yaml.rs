use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioYaml {
    /// Scenario name (metrics scenario tag).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Entry function name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<String>,

    /// Scenario-level tags.
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        default,
        deserialize_with = "deserialize_tags"
    )]
    pub tags: BTreeMap<String, String>,

    /// Executor kind: constant-vus | ramping-vus | ramping-arrival-rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executor: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub vus: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iterations: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration: Option<YamlDuration>,

    // ramping-vus
    #[serde(rename = "startVUs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_vus: Option<u64>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub stages: Vec<StageYaml>,

    // ramping-arrival-rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_rate: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub time_unit: Option<YamlDuration>,

    #[serde(rename = "preAllocatedVUs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_allocated_vus: Option<u64>,

    #[serde(rename = "maxVUs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_vus: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StageYaml {
    pub target: u64,

    #[serde(default)]
    pub duration: YamlDuration,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct YamlDuration(Duration);

impl YamlDuration {
    fn into_inner(self) -> Duration {
        self.0
    }
}

impl From<Duration> for YamlDuration {
    fn from(value: Duration) -> Self {
        Self(value)
    }
}

impl Serialize for YamlDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&humantime::format_duration(self.0).to_string())
    }
}

impl<'de> Deserialize<'de> for YamlDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = YamlDuration;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("duration as string (e.g. 10s), integer seconds, or float seconds")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(YamlDuration(Duration::from_secs(v)))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v <= 0 {
                    return Err(E::custom("duration must be positive"));
                }
                Ok(YamlDuration(Duration::from_secs(v as u64)))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if !v.is_finite() || v <= 0.0 {
                    return Err(E::custom("duration must be a positive, finite number"));
                }
                Ok(YamlDuration(Duration::from_secs_f64(v)))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let d = humantime::parse_duration(v).map_err(E::custom)?;
                Ok(YamlDuration(d))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&v)
            }
        }

        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioDocYamlNested {
    pub scenario: ScenarioYaml,

    #[serde(default)]
    pub thresholds: BTreeMap<String, ThresholdExprYaml>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioDocYamlFlat {
    #[serde(flatten)]
    pub scenario: ScenarioYaml,

    #[serde(default)]
    pub thresholds: BTreeMap<String, ThresholdExprYaml>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioDocYamlMulti {
    pub scenarios: Vec<ScenarioYaml>,

    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub thresholds: BTreeMap<String, ThresholdExprYaml>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ScenarioDocYaml {
    Multi(ScenarioDocYamlMulti),
    Nested(ScenarioDocYamlNested),
    Flat(ScenarioDocYamlFlat),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ThresholdExprYaml {
    One(String),
    Many(Vec<String>),
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = BTreeMap::<String, serde_yaml::Value>::deserialize(deserializer)?;
    let mut out = BTreeMap::new();

    for (k, v) in raw {
        let s = match v {
            serde_yaml::Value::Null => continue,
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::String(s) => s,
            _ => continue,
        };
        out.insert(k, s);
    }

    Ok(out)
}

pub fn looks_like_yaml_path(raw: &str) -> bool {
    let p = Path::new(raw);
    matches!(
        p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()),
        Some(ext) if ext == "yml" || ext == "yaml"
    )
}

pub async fn load_script_options_from_yaml(
    path: &Path,
) -> anyhow::Result<wrkr_core::ScriptOptions> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("failed to read scenario YAML: {}", path.display()))?;

    let doc: ScenarioDocYaml = serde_yaml::from_slice(&bytes)
        .with_context(|| format!("failed to parse YAML: {}", path.display()))?;

    let (scenarios_yaml, thresholds) = match doc {
        ScenarioDocYaml::Multi(d) => (d.scenarios, d.thresholds),
        ScenarioDocYaml::Nested(d) => (vec![d.scenario], d.thresholds),
        ScenarioDocYaml::Flat(d) => (vec![d.scenario], d.thresholds),
    };

    let total = scenarios_yaml.len();
    let scenarios = scenarios_yaml
        .into_iter()
        .enumerate()
        .map(|(idx, scenario)| {
            let name_opt = scenario.name.clone();

            let default_name = if total <= 1 {
                name_opt
                    .clone()
                    .or_else(|| {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "main".to_string())
            } else {
                name_opt.unwrap_or_else(|| format!("scenario_{}", idx + 1))
            };

            scenario_yaml_into_options(scenario, default_name)
        })
        .collect::<Vec<_>>();

    let thresholds = parse_thresholds_map(thresholds)?;

    Ok(wrkr_core::ScriptOptions {
        vus: None,
        iterations: None,
        duration: None,
        scenarios,
        thresholds,
    })
}

fn scenario_yaml_into_options(
    scenario: ScenarioYaml,
    default_name: String,
) -> wrkr_core::ScenarioOptions {
    let ScenarioYaml {
        name,
        exec,
        tags,
        executor,
        vus,
        iterations,
        duration,
        start_vus,
        stages,
        start_rate,
        time_unit,
        pre_allocated_vus,
        max_vus,
    } = scenario;

    let name = name.unwrap_or(default_name);
    let tags = tags.into_iter().collect::<Vec<_>>();

    wrkr_core::ScenarioOptions {
        name,
        exec,
        tags,
        executor,
        vus,
        iterations,
        duration: duration.map(|d| d.into_inner()),

        start_vus,
        stages: stages
            .into_iter()
            .map(|s| wrkr_core::Stage {
                duration: s.duration.into_inner(),
                target: s.target,
            })
            .collect(),

        start_rate,
        time_unit: time_unit.map(|d| d.into_inner()),
        pre_allocated_vus,
        max_vus,
    }
}

pub(crate) fn build_doc_from_resolved_scenario(
    s: &wrkr_core::ScenarioConfig,
    thresholds: &[wrkr_core::ThresholdSet],
) -> ScenarioDocYamlFlat {
    let mut tags = BTreeMap::new();
    for (k, v) in s.metrics_ctx.scenario_tags().iter() {
        tags.insert(k.clone(), v.clone());
    }

    let scenario = match &s.executor {
        wrkr_core::ScenarioExecutor::ConstantVus { vus } => ScenarioYaml {
            name: Some(s.metrics_ctx.scenario().to_string()),
            exec: Some(s.exec.clone()),
            tags,
            executor: Some("constant-vus".to_string()),
            vus: Some(*vus),
            iterations: s.iterations,
            duration: s.duration.map(YamlDuration::from),
            start_vus: None,
            stages: Vec::new(),
            start_rate: None,
            time_unit: None,
            pre_allocated_vus: None,
            max_vus: None,
        },
        wrkr_core::ScenarioExecutor::RampingVus { start_vus, stages } => ScenarioYaml {
            name: Some(s.metrics_ctx.scenario().to_string()),
            exec: Some(s.exec.clone()),
            tags,
            executor: Some("ramping-vus".to_string()),
            vus: None,
            iterations: None,
            duration: None,
            start_vus: Some(*start_vus),
            stages: stages
                .iter()
                .map(|st| StageYaml {
                    duration: YamlDuration::from(st.duration),
                    target: st.target,
                })
                .collect(),
            start_rate: None,
            time_unit: None,
            pre_allocated_vus: None,
            max_vus: None,
        },
        wrkr_core::ScenarioExecutor::RampingArrivalRate {
            start_rate,
            time_unit,
            pre_allocated_vus,
            max_vus,
            stages,
        } => ScenarioYaml {
            name: Some(s.metrics_ctx.scenario().to_string()),
            exec: Some(s.exec.clone()),
            tags,
            executor: Some("ramping-arrival-rate".to_string()),
            vus: None,
            iterations: None,
            duration: None,
            start_vus: None,
            stages: stages
                .iter()
                .map(|st| StageYaml {
                    duration: YamlDuration::from(st.duration),
                    target: st.target,
                })
                .collect(),
            start_rate: Some(*start_rate),
            time_unit: Some(YamlDuration::from(*time_unit)),
            pre_allocated_vus: Some(*pre_allocated_vus),
            max_vus: Some(*max_vus),
        },
    };

    ScenarioDocYamlFlat {
        scenario,
        thresholds: render_thresholds(thresholds),
    }
}

pub(crate) fn build_doc_from_resolved_scenarios(
    scenarios: &[wrkr_core::ScenarioConfig],
    thresholds: &[wrkr_core::ThresholdSet],
) -> ScenarioDocYamlMulti {
    let scenarios = scenarios
        .iter()
        .map(|s| build_doc_from_resolved_scenario(s, &[]).scenario)
        .collect::<Vec<_>>();

    ScenarioDocYamlMulti {
        scenarios,
        thresholds: render_thresholds(thresholds),
    }
}

pub(crate) async fn write_yaml_file<T: Serialize>(path: &Path, doc: &T) -> anyhow::Result<()> {
    let s = serde_yaml::to_string(doc).context("failed to serialize YAML")?;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    tokio::fs::write(path, s)
        .await
        .with_context(|| format!("failed to write file: {}", path.display()))?;

    Ok(())
}

fn render_thresholds(sets: &[wrkr_core::ThresholdSet]) -> BTreeMap<String, ThresholdExprYaml> {
    let mut out = BTreeMap::new();

    for s in sets {
        let key = render_metric_key(&s.metric, &s.tags);
        let v = if s.expressions.len() == 1 {
            ThresholdExprYaml::One(s.expressions[0].clone())
        } else {
            ThresholdExprYaml::Many(s.expressions.clone())
        };
        out.insert(key, v);
    }

    out
}

fn render_metric_key(metric: &str, tags: &[(String, String)]) -> String {
    if tags.is_empty() {
        return metric.to_string();
    }

    let selector = tags
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");

    format!("{metric}{{{selector}}}")
}

fn parse_thresholds_map(
    raw: BTreeMap<String, ThresholdExprYaml>,
) -> anyhow::Result<Vec<wrkr_core::ThresholdSet>> {
    let mut out = Vec::new();

    for (metric_key, v) in raw {
        let (metric, tags) = wrkr_core::parse_threshold_metric_key(&metric_key)
            .map_err(|e| anyhow::anyhow!("invalid threshold metric key `{metric_key}`: {e}"))?;

        let expressions: Vec<String> = match v {
            ThresholdExprYaml::One(s) => vec![s],
            ThresholdExprYaml::Many(v) => v,
        };

        if expressions.is_empty() {
            anyhow::bail!("invalid thresholds for `{metric_key}`: empty list");
        }

        out.push(wrkr_core::ThresholdSet {
            metric,
            tags,
            expressions,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("scenario_yaml")
            .join(name)
    }

    #[test]
    fn looks_like_yaml_path_checks_extension() {
        assert!(looks_like_yaml_path("a.yml"));
        assert!(looks_like_yaml_path("a.yaml"));
        assert!(!looks_like_yaml_path("a.lua"));
        assert!(!looks_like_yaml_path("main"));
    }

    #[test]
    fn tags_deserialize_simple_scalars_as_strings() {
        let doc: ScenarioDocYamlFlat = serde_yaml::from_str(
            r#"
name: main
exec: Default
tags:
  a: true
  b: 123
  c: x
"#,
        )
        .unwrap_or_else(|e| panic!("{e:#}"));

        assert_eq!(doc.scenario.tags.get("a").map(String::as_str), Some("true"));
        assert_eq!(doc.scenario.tags.get("b").map(String::as_str), Some("123"));
        assert_eq!(doc.scenario.tags.get("c").map(String::as_str), Some("x"));
    }

    #[test]
    fn metric_key_renders_selector() {
        let key = render_metric_key(
            "http_req_duration",
            &[
                ("group".to_string(), "login".to_string()),
                ("method".to_string(), "GET".to_string()),
            ],
        );
        assert_eq!(key, "http_req_duration{group=login,method=GET}");
    }

    #[tokio::test]
    async fn loads_flat_yaml() {
        let path = fixture_path("flat.yaml");
        let opts = load_script_options_from_yaml(&path)
            .await
            .unwrap_or_else(|e| panic!("{e:#}"));

        assert_eq!(opts.scenarios.len(), 1);
        let s = &opts.scenarios[0];
        assert_eq!(s.name, "main");
        assert_eq!(s.exec.as_deref(), Some("Default"));
        assert_eq!(s.executor.as_deref(), Some("constant-vus"));
        assert_eq!(s.vus, Some(5));
        assert!(opts.thresholds.len() == 1);
    }

    #[tokio::test]
    async fn loads_nested_yaml() {
        let path = fixture_path("nested.yaml");
        let opts = load_script_options_from_yaml(&path)
            .await
            .unwrap_or_else(|e| panic!("{e:#}"));

        assert_eq!(opts.scenarios.len(), 1);
        let s = &opts.scenarios[0];
        assert_eq!(s.name, "main");
        assert_eq!(s.executor.as_deref(), Some("ramping-vus"));
        assert_eq!(s.start_vus, Some(0));
        assert_eq!(s.stages.len(), 2);
    }

    #[tokio::test]
    async fn loads_multi_yaml() {
        let path = fixture_path("multi.yaml");
        let opts = load_script_options_from_yaml(&path)
            .await
            .unwrap_or_else(|e| panic!("{e:#}"));

        assert_eq!(opts.scenarios.len(), 2);
        assert_eq!(opts.scenarios[0].name, "main");
        assert_eq!(opts.scenarios[1].name, "alt");
        assert_eq!(opts.thresholds.len(), 2);
    }

    #[tokio::test]
    async fn export_then_import_roundtrips_executor_kinds() {
        fn arc_tags(tags: Vec<(String, String)>) -> Arc<[(String, String)]> {
            Arc::from(tags.into_boxed_slice())
        }

        let const_name: Arc<str> = Arc::from("const");
        let ramp_name: Arc<str> = Arc::from("ramp");
        let rate_name: Arc<str> = Arc::from("rate");

        let const_cfg = wrkr_core::ScenarioConfig {
            exec: "Default".to_string(),
            metrics_ctx: wrkr_core::MetricsContext::new(
                const_name.clone(),
                arc_tags(vec![("tier".to_string(), "core".to_string())]),
            ),
            executor: wrkr_core::ScenarioExecutor::ConstantVus { vus: 5 },
            iterations: Some(10),
            duration: Some(Duration::from_secs(2)),
        };

        let ramp_stages = vec![
            wrkr_core::Stage {
                duration: Duration::from_secs(1),
                target: 3,
            },
            wrkr_core::Stage {
                duration: Duration::from_secs(2),
                target: 1,
            },
        ];
        let ramp_total = ramp_stages
            .iter()
            .fold(Duration::ZERO, |acc, st| acc.saturating_add(st.duration));

        let ramp_cfg = wrkr_core::ScenarioConfig {
            exec: "Ramp".to_string(),
            metrics_ctx: wrkr_core::MetricsContext::new(ramp_name.clone(), arc_tags(vec![])),
            executor: wrkr_core::ScenarioExecutor::RampingVus {
                start_vus: 0,
                stages: ramp_stages,
            },
            iterations: None,
            duration: Some(ramp_total),
        };

        let rate_stages = vec![
            wrkr_core::Stage {
                duration: Duration::from_secs(1),
                target: 10,
            },
            wrkr_core::Stage {
                duration: Duration::from_secs(3),
                target: 20,
            },
        ];
        let rate_total = rate_stages
            .iter()
            .fold(Duration::ZERO, |acc, st| acc.saturating_add(st.duration));

        let rate_cfg = wrkr_core::ScenarioConfig {
            exec: "Rate".to_string(),
            metrics_ctx: wrkr_core::MetricsContext::new(rate_name.clone(), arc_tags(vec![])),
            executor: wrkr_core::ScenarioExecutor::RampingArrivalRate {
                start_rate: 5,
                time_unit: Duration::from_secs(1),
                pre_allocated_vus: 2,
                max_vus: 5,
                stages: rate_stages,
            },
            iterations: None,
            duration: Some(rate_total),
        };

        let thresholds = vec![wrkr_core::ThresholdSet {
            metric: "http_req_duration".to_string(),
            tags: vec![("scenario".to_string(), "const".to_string())],
            expressions: vec!["p(95)<200".to_string()],
        }];

        let resolved = vec![const_cfg.clone(), ramp_cfg.clone(), rate_cfg.clone()];

        // Export: always multi-scenario YAML.
        let doc = build_doc_from_resolved_scenarios(&resolved, &thresholds);
        let yaml = serde_yaml::to_string(&doc).unwrap_or_else(|e| panic!("{e:#}"));
        let parsed_multi: ScenarioDocYamlMulti =
            serde_yaml::from_str(&yaml).unwrap_or_else(|e| panic!("{e:#}"));
        assert_eq!(parsed_multi.scenarios.len(), 3);

        // Import: parse YAML file into ScriptOptions.
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|e| panic!("time went backwards: {e:#}"))
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!(
            "wrkr_scenario_roundtrip_{}_{}.yaml",
            std::process::id(),
            ts
        ));

        write_yaml_file(&tmp, &doc)
            .await
            .unwrap_or_else(|e| panic!("{e:#}"));

        let imported_opts = load_script_options_from_yaml(&tmp)
            .await
            .unwrap_or_else(|e| panic!("{e:#}"));
        let _ = tokio::fs::remove_file(&tmp).await;

        assert_eq!(imported_opts.scenarios.len(), 3);
        assert_eq!(imported_opts.thresholds.len(), 1);

        // Validate thresholds roundtrip.
        let mut got_thr = imported_opts.thresholds.clone();
        let mut exp_thr = thresholds.clone();
        for t in &mut got_thr {
            t.tags.sort();
        }
        for t in &mut exp_thr {
            t.tags.sort();
        }
        got_thr.sort_by(|a, b| (a.metric.as_str(), &a.tags).cmp(&(b.metric.as_str(), &b.tags)));
        exp_thr.sort_by(|a, b| (a.metric.as_str(), &a.tags).cmp(&(b.metric.as_str(), &b.tags)));
        assert_eq!(got_thr.len(), exp_thr.len());
        assert_eq!(got_thr[0].metric, exp_thr[0].metric);
        assert_eq!(got_thr[0].tags, exp_thr[0].tags);
        assert_eq!(got_thr[0].expressions, exp_thr[0].expressions);

        // Resolve imported options into ScenarioConfig and compare executors.
        let imported_cfgs = wrkr_core::scenarios_from_options(
            imported_opts,
            wrkr_core::RunConfig {
                iterations: None,
                vus: None,
                duration: None,
            },
        )
        .unwrap_or_else(|e| panic!("{e:#}"));

        assert_eq!(imported_cfgs.len(), 3);
        for got in imported_cfgs {
            let expected = resolved
                .iter()
                .find(|s| s.metrics_ctx.scenario() == got.metrics_ctx.scenario())
                .unwrap_or_else(|| panic!("missing scenario: {}", got.metrics_ctx.scenario()));

            assert_eq!(got.exec, expected.exec);
            assert_eq!(got.iterations, expected.iterations);
            assert_eq!(got.duration, expected.duration);
            assert_eq!(
                got.metrics_ctx.scenario_tags(),
                expected.metrics_ctx.scenario_tags()
            );

            match (&got.executor, &expected.executor) {
                (
                    wrkr_core::ScenarioExecutor::ConstantVus { vus: a },
                    wrkr_core::ScenarioExecutor::ConstantVus { vus: b },
                ) => assert_eq!(a, b),
                (
                    wrkr_core::ScenarioExecutor::RampingVus {
                        start_vus: a_start,
                        stages: a_st,
                    },
                    wrkr_core::ScenarioExecutor::RampingVus {
                        start_vus: b_start,
                        stages: b_st,
                    },
                ) => {
                    assert_eq!(a_start, b_start);
                    assert_eq!(a_st.len(), b_st.len());
                    for (a, b) in a_st.iter().zip(b_st.iter()) {
                        assert_eq!(a.target, b.target);
                        assert_eq!(a.duration, b.duration);
                    }
                }
                (
                    wrkr_core::ScenarioExecutor::RampingArrivalRate {
                        start_rate: a_rate,
                        time_unit: a_unit,
                        pre_allocated_vus: a_pre,
                        max_vus: a_max,
                        stages: a_st,
                    },
                    wrkr_core::ScenarioExecutor::RampingArrivalRate {
                        start_rate: b_rate,
                        time_unit: b_unit,
                        pre_allocated_vus: b_pre,
                        max_vus: b_max,
                        stages: b_st,
                    },
                ) => {
                    assert_eq!(a_rate, b_rate);
                    assert_eq!(a_unit, b_unit);
                    assert_eq!(a_pre, b_pre);
                    assert_eq!(a_max, b_max);
                    assert_eq!(a_st.len(), b_st.len());
                    for (a, b) in a_st.iter().zip(b_st.iter()) {
                        assert_eq!(a.target, b.target);
                        assert_eq!(a.duration, b.duration);
                    }
                }
                _ => panic!("executor mismatch for {}", got.metrics_ctx.scenario()),
            }
        }
    }
}
