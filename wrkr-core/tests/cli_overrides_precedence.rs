use std::time::Duration;

use wrkr_core::runner::{
    RunConfig, ScenarioExecutor, ScenarioOptions, ScriptOptions, Stage, scenarios_from_options,
};

#[test]
fn cli_overrides_convert_ramping_vus_to_constant_vus() {
    let opts = ScriptOptions {
        scenarios: vec![ScenarioOptions {
            name: "HighLoad".to_string(),
            exec: Some("HighLoad".to_string()),
            executor: Some("ramping-vus".to_string()),
            // Intentionally invalid/missing ramping fields. We should ignore these
            // when CLI overrides are present.
            stages: vec![],
            start_vus: None,
            vus: None,
            iterations: None,
            duration: None,
            start_rate: None,
            time_unit: None,
            pre_allocated_vus: None,
            max_vus: None,
        }],
        ..ScriptOptions::default()
    };

    let cfg = RunConfig {
        iterations: Some(1),
        vus: Some(1),
        duration: None,
    };

    let scenarios = scenarios_from_options(opts, cfg)
        .unwrap_or_else(|e| panic!("expected scenarios to be valid: {e}"));
    assert_eq!(scenarios.len(), 1);

    let s = &scenarios[0];
    assert_eq!(s.name, "HighLoad");
    assert_eq!(s.exec, "HighLoad");
    assert_eq!(s.iterations, Some(1));
    assert_eq!(s.duration, None);

    match s.executor {
        ScenarioExecutor::ConstantVus { vus } => assert_eq!(vus, 1),
        _ => panic!("expected constant-vus executor"),
    }
}

#[test]
fn ramping_vus_still_validates_when_no_cli_overrides() {
    let opts = ScriptOptions {
        scenarios: vec![ScenarioOptions {
            name: "HighLoad".to_string(),
            exec: Some("HighLoad".to_string()),
            executor: Some("ramping-vus".to_string()),
            stages: vec![Stage {
                duration: Duration::from_secs(1),
                target: 2,
            }],
            start_vus: Some(1),
            vus: None,
            iterations: None,
            duration: None,
            start_rate: None,
            time_unit: None,
            pre_allocated_vus: None,
            max_vus: None,
        }],
        ..ScriptOptions::default()
    };

    let cfg = RunConfig::default();

    let scenarios = scenarios_from_options(opts, cfg)
        .unwrap_or_else(|e| panic!("expected scenarios to be valid: {e}"));
    assert_eq!(scenarios.len(), 1);

    match &scenarios[0].executor {
        ScenarioExecutor::RampingVus { start_vus, stages } => {
            assert_eq!(*start_vus, 1);
            assert_eq!(stages.len(), 1);
        }
        _ => panic!("expected ramping-vus executor"),
    }
}

#[test]
fn cli_overrides_convert_ramping_arrival_rate_to_constant_vus() {
    let opts = ScriptOptions {
        scenarios: vec![ScenarioOptions {
            name: "HighRps".to_string(),
            exec: Some("HighRps".to_string()),
            executor: Some("ramping-arrival-rate".to_string()),
            // Intentionally invalid/missing arrival rate fields.
            stages: vec![],
            start_vus: None,
            vus: None,
            iterations: None,
            duration: None,
            start_rate: None,
            time_unit: None,
            pre_allocated_vus: None,
            max_vus: None,
        }],
        ..ScriptOptions::default()
    };

    let cfg = RunConfig {
        iterations: Some(1),
        vus: Some(1),
        duration: None,
    };

    let scenarios = scenarios_from_options(opts, cfg)
        .unwrap_or_else(|e| panic!("expected scenarios to be valid: {e}"));
    assert_eq!(scenarios.len(), 1);

    match scenarios[0].executor {
        ScenarioExecutor::ConstantVus { vus } => assert_eq!(vus, 1),
        _ => panic!("expected constant-vus executor"),
    }
}
