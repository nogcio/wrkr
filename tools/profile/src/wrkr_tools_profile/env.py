from __future__ import annotations

from .errors import ProfileError


def format_env_templates(templates: tuple[str, ...], *, http_url: str, grpc_url: str) -> list[str]:
    mapping = {
        "HTTP_URL": http_url,
        "GRPC_URL": grpc_url,
    }

    out: list[str] = []
    for item in templates:
        if "=" not in item:
            raise ProfileError(f"Invalid env template (expected KEY=VALUE): {item!r}")
        key, value_tmpl = item.split("=", 1)
        key = key.strip()
        value_tmpl = value_tmpl.strip()
        if key == "":
            raise ProfileError(f"Invalid env template (empty key): {item!r}")
        value = value_tmpl.format_map(mapping)
        out.append(f"{key}={value}")
    return out
