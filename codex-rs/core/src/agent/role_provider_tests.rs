use super::role::apply_role_to_config;
use crate::config::AgentRoleConfig;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use tempfile::TempDir;

#[tokio::test]
async fn user_allowlisted_role_can_switch_model_provider() {
    let home = TempDir::new().expect("create temp dir");
    let home_path = home.path().to_path_buf();
    let mut config = ConfigBuilder::default()
        .codex_home(home_path.clone())
        .fallback_cwd(Some(home_path))
        .build()
        .await
        .expect("load test config");

    let provider_id = "custom";
    let custom_provider =
        create_oss_provider_with_base_url("http://127.0.0.1:12345/v1", WireApi::Responses);
    config
        .model_providers
        .insert(provider_id.to_string(), custom_provider.clone());
    config.config_layer_stack = config
        .config_layer_stack
        .with_user_config(
            &config.codex_home.join(CONFIG_TOML_FILE),
            toml::from_str(&format!(
                "subagent_model_provider_allowlist = [\"{provider_id}\"]"
            ))
            .expect("valid user config"),
        )
        .expect("user config should be valid");

    let role_path = config.codex_home.join("custom-provider-role.toml");
    tokio::fs::write(
        role_path.as_path(),
        "model = \"external-model\"\nmodel_provider = \"custom\"\n",
    )
    .await
    .expect("write role config");
    config.agent_roles.insert(
        "custom-child".to_string(),
        AgentRoleConfig {
            description: Some("Custom provider child".to_string()),
            config_file: Some(role_path.to_path_buf()),
            model_provider: Some(provider_id.to_string()),
            model_catalog_json: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom-child"))
        .await
        .expect("trusted role should apply");

    assert_eq!(config.model_provider_id, provider_id);
    assert_eq!(config.model_provider, custom_provider);
    assert_eq!(config.model.as_deref(), Some("external-model"));
}
