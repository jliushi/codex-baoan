use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(super) fn codex_home() -> PathBuf {
    resolve_codex_home(
        &dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
        std::env::var_os("CODEX_HOME").as_deref(),
    )
}

fn resolve_codex_home(home: &Path, override_path: Option<&OsStr>) -> PathBuf {
    override_path
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"))
}

pub(super) struct ResolvedConfig {
    pub provider_name: String,
    pub name: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub protocol: String,
    pub status: String,
    pub status_text: String,
    pub notes: Vec<String>,
}

fn text(value: Option<&toml::Value>) -> Option<String> {
    value
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn json_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn credential_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "x-api-key" | "api-key" | "apikey"
    )
}

pub(super) fn resolve(
    raw: &str,
    auth: &Value,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<ResolvedConfig, String> {
    let config: toml::Value = raw.parse().map_err(|error: toml::de::Error| {
        let line = error
            .span()
            .map(|span| {
                raw[..span.start]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1
            })
            .unwrap_or(1);
        format!("Codex config.toml 第 {line} 行格式错误。")
    })?;
    let profile_name = text(config.get("profile"));
    let profile = match profile_name.as_ref() {
        Some(name) => Some(
            config
                .get("profiles")
                .and_then(|profiles| profiles.get(name))
                .filter(|profile| profile.is_table())
                .ok_or_else(|| "Codex config.toml 指定的 profile 不存在。".to_string())?,
        ),
        None => None,
    };
    let setting = |key| {
        profile
            .and_then(|profile| profile.get(key))
            .or_else(|| config.get(key))
    };
    let provider_name = text(setting("model_provider")).unwrap_or_else(|| "openai".into());
    let provider = config
        .get("model_providers")
        .and_then(|providers| providers.get(&provider_name));
    let builtin = matches!(
        provider_name.as_str(),
        "openai" | "ollama" | "lmstudio" | "amazon-bedrock" | "amazon-bedrock-runtime"
    );
    if provider.is_none() && !builtin {
        return Err("Codex 当前 model_provider 没有对应的供应商配置。".into());
    }
    // Built-in OpenAI/OSS definitions are authoritative in current Codex.
    let provider = if matches!(provider_name.as_str(), "openai" | "ollama" | "lmstudio") {
        None
    } else {
        provider
    };
    let field = |key| provider.and_then(|provider| provider.get(key));
    let requires_login = provider_name == "openai"
        || field("requires_openai_auth")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
    let protocol = text(field("wire_api")).unwrap_or_else(|| "responses".into());
    let mut notes = vec![];
    if let Some(profile) = profile_name {
        notes.push(format!("profile: {profile}"));
    }
    let auth_store =
        text(config.get("cli_auth_credentials_store")).unwrap_or_else(|| "file".into());
    let external_store = matches!(auth_store.as_str(), "keyring" | "auto" | "ephemeral");
    let saved_auth = if external_store { &Value::Null } else { auth };
    let auth_mode = saved_auth["auth_mode"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace(['_', '-'], "");
    let chatgpt = requires_login
        && auth_mode != "apikey"
        && ((json_text(&saved_auth["tokens"]["access_token"]).is_some()
            && (matches!(auth_mode.as_str(), "chatgpt" | "chatgptauthtokens")
                || saved_auth["OPENAI_API_KEY"].as_str().is_none()))
            || json_text(&saved_auth["personal_access_token"]).is_some()
            || !saved_auth["agent_identity"].is_null());

    let env_key = text(field("env_key"));
    let api_key = if let Some(env_key) = env_key.as_deref() {
        nonempty(env(env_key))
    } else {
        text(field("experimental_bearer_token")).or_else(|| {
            if requires_login && !chatgpt {
                json_text(&saved_auth["OPENAI_API_KEY"])
            } else {
                None
            }
        })
    };
    let has_header = field("http_headers")
        .and_then(toml::Value::as_table)
        .is_some_and(|headers| {
            headers
                .iter()
                .any(|(name, value)| credential_header(name) && text(Some(value)).is_some())
        })
        || field("env_http_headers")
            .and_then(toml::Value::as_table)
            .is_some_and(|headers| {
                headers.iter().any(|(name, value)| {
                    credential_header(name)
                        && value.as_str().and_then(|key| nonempty(env(key))).is_some()
                })
            });
    let external_auth = field("auth").is_some()
        || field("aws").is_some()
        || provider_name.starts_with("amazon-bedrock");

    let base_url = text(field("base_url"))
        .or_else(|| match provider_name.as_str() {
            "openai" => nonempty(env("OPENAI_BASE_URL")),
            "ollama" | "lmstudio" => {
                let port = nonempty(env("CODEX_OSS_PORT"))
                    .and_then(|port| port.parse::<u16>().ok())
                    .unwrap_or(if provider_name == "ollama" {
                        11434
                    } else {
                        1234
                    });
                nonempty(env("CODEX_OSS_BASE_URL"))
                    .or_else(|| Some(format!("http://localhost:{port}/v1")))
            }
            _ => None,
        })
        .or_else(|| {
            if provider_name.starts_with("amazon-bedrock") {
                None
            } else if chatgpt {
                Some("https://chatgpt.com/backend-api/codex".into())
            } else {
                Some("https://api.openai.com/v1".into())
            }
        });
    let valid_url = base_url.as_deref().is_some_and(|value| {
        url::Url::parse(value)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
    });
    let local = matches!(provider_name.as_str(), "ollama" | "lmstudio");
    let (status, status_text) = if !valid_url && !external_auth {
        ("unconfigured", "上游地址无效")
    } else if protocol != "responses" {
        notes.push("当前 Codex 仅支持 wire_api = responses。".into());
        ("unconfigured", "协议不兼容")
    } else if api_key.is_some() {
        ("ready", "已配置 API Key")
    } else if env_key.is_some() {
        ("needs-auth", "未检测到指定环境变量")
    } else if chatgpt {
        ("ready", "已发现 ChatGPT 登录态")
    } else if requires_login && external_store {
        ("auth-unverified", "外部登录态待确认")
    } else if external_auth {
        ("auth-unverified", "外部认证待确认")
    } else if has_header {
        ("ready", "已配置认证头")
    } else if local {
        ("ready", "本地模型")
    } else if !requires_login {
        ("auth-unverified", "未配置认证")
    } else {
        ("needs-auth", "未检测到登录凭据")
    };
    if requires_login && external_store {
        notes.push(format!(
            "cli_auth_credentials_store = {auth_store}；不读取系统凭据库，不验证远端登录有效性。"
        ));
    }
    if external_auth {
        notes.push("仅识别外部认证配置，不执行认证命令。".into());
    }
    let name = text(field("name")).unwrap_or_else(|| match provider_name.as_str() {
        "openai" if chatgpt => "OpenAI / ChatGPT".into(),
        "openai" => "OpenAI".into(),
        "ollama" => "Ollama".into(),
        "lmstudio" => "LM Studio".into(),
        _ => provider_name.clone(),
    });
    Ok(ResolvedConfig {
        provider_name,
        name,
        base_url: base_url.map(|url| redact_url(&url)),
        api_key,
        model: text(setting("model")),
        protocol,
        status: status.into(),
        status_text: status_text.into(),
        notes,
    })
}

/// 读取本机 Codex 配置(config.toml + auth.json)并解析，供模型探测复用。
/// 返回的 base_url 已脱敏但仍可用；api_key 为明文（仅在后端使用，勿回传前端）。
pub(super) fn resolve_active() -> Result<ResolvedConfig, String> {
    let root = codex_home();
    let raw = std::fs::read_to_string(root.join("config.toml")).unwrap_or_default();
    let auth = std::fs::read_to_string(root.join("auth.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or(Value::Null);
    resolve(&raw, &auth, &|name| std::env::var(name).ok())
}

/// 探测用的 bearer 凭据：API Key 优先，其次 ChatGPT 订阅登录态的 access_token。
/// 覆盖"用过 codex 订阅"的情况——订阅号没有 API Key，用 OAuth token 当 bearer 探测。
pub(super) fn active_bearer_token() -> Option<String> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key);
        }
    }
    let auth: Value = std::fs::read_to_string(codex_home().join("auth.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())?;
    json_text(&auth["OPENAI_API_KEY"])
        .or_else(|| json_text(&auth["tokens"]["access_token"]))
        .or_else(|| json_text(&auth["personal_access_token"]))
}

pub(super) fn redact_url(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return super::redact_command(value);
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    let pairs: Vec<_> = url
        .query_pairs()
        .map(|(key, value)| {
            let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
            let secret = matches!(
                normalized.as_str(),
                "apikey" | "token" | "accesstoken" | "password" | "passwd" | "secret" | "key"
            );
            (
                key.into_owned(),
                if secret {
                    "[REDACTED]".into()
                } else {
                    value.into_owned()
                },
            )
        })
        .collect();
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.set_fragment(None);
    url.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(raw: &str, auth: Value) -> ResolvedConfig {
        resolve(raw, &auth, &|_| None).unwrap()
    }

    #[test]
    fn respects_codex_home_override_and_empty_default() {
        let home = Path::new("test-home");
        assert_eq!(resolve_codex_home(home, None), home.join(".codex"));
        assert_eq!(
            resolve_codex_home(home, Some(OsStr::new(""))),
            home.join(".codex")
        );
        assert_eq!(
            resolve_codex_home(home, Some(OsStr::new("custom-home"))),
            PathBuf::from("custom-home")
        );
    }

    #[test]
    fn defaults_to_openai_instead_of_first_unused_provider() {
        let info = parse(
            "[model_providers.unused]\nbase_url = 'https://unused.example/v1'",
            json!({"OPENAI_API_KEY":"test-api-key"}),
        );
        assert_eq!(info.provider_name, "openai");
        assert_eq!(info.base_url.as_deref(), Some("https://api.openai.com/v1"));
        assert_eq!(info.status, "ready");
    }

    #[test]
    fn chatgpt_login_works_without_config_or_api_key() {
        let info = parse(
            "",
            json!({"auth_mode":"chatgpt","tokens":{"access_token":"private-oauth-token"}}),
        );
        assert_eq!(info.status, "ready");
        assert_eq!(
            info.base_url.as_deref(),
            Some("https://chatgpt.com/backend-api/codex")
        );
        assert!(info.api_key.is_none());
    }

    #[test]
    fn selected_profile_overrides_provider_and_model() {
        let info = parse("model = 'global-model'\nmodel_provider = 'unused'\nprofile = 'work'\n[profiles.work]\nmodel = 'profile-model'\nmodel_provider = 'custom'\n[model_providers.custom]\nbase_url = 'https://custom.example/v1'\nrequires_openai_auth = true", json!({"OPENAI_API_KEY":"private-key"}));
        assert_eq!(info.provider_name, "custom");
        assert_eq!(info.model.as_deref(), Some("profile-model"));
        assert_eq!(info.status, "ready");
        assert_eq!(info.api_key.as_deref(), Some("private-key"));
    }

    #[test]
    fn explicit_env_key_does_not_fall_back_to_unrelated_auth_json() {
        let raw = "model_provider = 'custom'\n[model_providers.custom]\nbase_url = 'https://custom.example/v1'\nenv_key = 'CUSTOM_KEY'";
        let auth = json!({"OPENAI_API_KEY":"unrelated-key"});
        let missing = resolve(raw, &auth, &|_| None).unwrap();
        assert_eq!(missing.status, "needs-auth");
        assert!(missing.api_key.is_none());
        let found = resolve(raw, &auth, &|key| {
            (key == "CUSTOM_KEY").then(|| "custom-key".into())
        })
        .unwrap();
        assert_eq!(found.api_key.as_deref(), Some("custom-key"));
        assert_eq!(found.status, "ready");
    }

    #[test]
    fn reports_external_auth_without_using_stale_file_credentials() {
        let info = parse(
            "cli_auth_credentials_store = 'keyring'",
            json!({"OPENAI_API_KEY":"stale-key"}),
        );
        assert_eq!(info.status, "auth-unverified");
        assert!(info.api_key.is_none());
        let command = parse("model_provider = 'custom'\n[model_providers.custom]\nbase_url = 'https://custom.example/v1'\n[model_providers.custom.auth]\ncommand = 'do-not-execute'", Value::Null);
        assert_eq!(command.status, "auth-unverified");
    }

    #[test]
    fn local_models_do_not_require_api_keys() {
        let info = parse("model_provider = 'ollama'", Value::Null);
        assert_eq!(info.status, "ready");
        assert_eq!(info.base_url.as_deref(), Some("http://localhost:11434/v1"));
    }

    #[test]
    fn errors_are_visible_without_echoing_secret_values() {
        let error = resolve(
            "experimental_bearer_token = 'private-key'\ninvalid = [",
            &Value::Null,
            &|_| None,
        )
        .err()
        .unwrap();
        assert!(!error.contains("private-key"));
        assert!(resolve("profile = 'missing'", &Value::Null, &|_| None).is_err());
        assert!(resolve("model_provider = 'missing'", &Value::Null, &|_| None).is_err());
    }

    #[test]
    fn redacts_url_credentials() {
        let url = redact_url("https://user:private-password@example.test/v1?api_key=private-key&region=cn#private-fragment");
        assert!(!url.contains("private-"));
        assert!(url.contains("region=cn"));
    }
}
