use url::Url;

fn env_get<'a>(env: &'a wrkr_core::EnvVars, key: &str) -> Option<&'a str> {
    env.iter()
        .find_map(|(k, v)| (k.as_ref() == key).then_some(v.as_ref()))
}

pub(super) fn resolve_base_url(env: &wrkr_core::EnvVars, url: String) -> String {
    // If the caller provided an absolute URL, keep it.
    if url.contains("://") {
        return url;
    }

    let Some(base) = env_get(env, "BASE_URL") else {
        return url;
    };

    let Ok(base_url) = Url::parse(base) else {
        return url;
    };

    base_url.join(&url).map(|u| u.to_string()).unwrap_or(url)
}

pub(super) fn apply_params_owned(url: String, params: &[(String, String)]) -> String {
    if params.is_empty() {
        return url;
    }

    let Ok(mut u) = Url::parse(&url) else {
        return url;
    };

    {
        let mut qp = u.query_pairs_mut();
        for (k, v) in params {
            qp.append_pair(k, v);
        }
    }

    u.to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::sync::Arc;

    #[test]
    fn resolve_base_url_keeps_absolute() {
        let env: wrkr_core::EnvVars = Arc::from([]);
        let url = "https://example.com/x".to_string();
        assert_eq!(resolve_base_url(&env, url.clone()), url);
    }

    #[test]
    fn resolve_base_url_joins_with_base_url() {
        let env: wrkr_core::EnvVars = Arc::from([(
            Arc::<str>::from("BASE_URL"),
            Arc::<str>::from("https://example.com/api/"),
        )]);

        assert_eq!(
            resolve_base_url(&env, "v1".to_string()),
            "https://example.com/api/v1"
        );
    }

    #[test]
    fn apply_params_owned_appends_query_pairs() {
        let url = "https://example.com/path".to_string();
        let out = apply_params_owned(
            url,
            &[
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
            ],
        );

        // order is deterministic with query_pairs_mut
        assert_eq!(out, "https://example.com/path?a=1&b=2");
    }
}
