//! Tests of the `env` commands.

use super::*;
use crate::test_utils::{mock_nodejs_app, ConfigBuilder, TempConfigDir};

/// Two projects with an app named `api`; the second one also answers to `shop-api`.
fn two_apis(dir: &TempConfigDir) {
    let mut shop_api = mock_nodejs_app("/tmp/shop-api");
    shop_api.alternative_name = Some("shop-api".into());
    let config = ConfigBuilder::new()
        .with_app("blog", "api", mock_nodejs_app("/tmp/blog-api"))
        .with_app("shop", "api", shop_api)
        .with_app("shop", "web", mock_nodejs_app("/tmp/shop-web"))
        .build();
    dir.write_config(&config);
}

fn env_files_of(project: &str, app: &str) -> Option<HashMap<String, HashMap<String, String>>> {
    load_config().unwrap().projects[project].apps[app]
        .env_files
        .clone()
}

#[test]
fn an_ambiguous_name_is_an_error_not_the_first_app() {
    // On a terminal an ambiguous name prompts instead.
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return;
    }
    let dir = TempConfigDir::new();
    two_apis(&dir);

    let err = add_env_file("api", None, "dev", "local", ".env.dev")
        .unwrap_err()
        .to_string();
    assert!(err.contains("ambiguous"), "{err}");
    assert!(env_files_of("blog", "api").is_none());
    assert!(env_files_of("shop", "api").is_none());
}

#[test]
fn project_flag_project_slash_app_and_alternative_name_pick_the_app() {
    let dir = TempConfigDir::new();
    two_apis(&dir);

    add_env_file("api", Some("shop"), "dev", "local", ".env.dev").unwrap();
    assert_eq!(
        env_files_of("shop", "api").unwrap()["dev"]["local"],
        ".env.dev"
    );
    assert!(env_files_of("blog", "api").is_none());

    add_env_file("blog/api", None, "qa", "local", ".env.qa").unwrap();
    assert_eq!(
        env_files_of("blog", "api").unwrap()["qa"]["local"],
        ".env.qa"
    );

    set_default_stage("shop-api", None, "local", "dev").unwrap();
    let config = load_config().unwrap();
    assert_eq!(
        config.projects["shop"].apps["api"]
            .default_stages
            .as_ref()
            .unwrap()["local"],
        "dev"
    );

    remove_env_file("shop-api", None, "dev", Some("local")).unwrap();
    assert!(env_files_of("shop", "api").is_none());
    assert!(list_env_files("api", Some("blog")).is_ok());
    assert!(list_env_files("nosuch", None).is_err());
}

#[test]
fn add_writes_only_the_given_context_and_remove_undoes_it() {
    let dir = TempConfigDir::new();
    two_apis(&dir);

    add_env_file("web", None, "dev", "orbstack", ".env.orb").unwrap();
    add_env_file("web", None, "dev", "docker", ".env.docker").unwrap();
    let files = env_files_of("shop", "web").unwrap();
    assert_eq!(files["dev"].len(), 2, "{files:?}");
    assert_eq!(files["dev"]["orbstack"], ".env.orb");
    assert_eq!(files["dev"]["docker"], ".env.docker");

    remove_env_file("web", None, "dev", Some("docker")).unwrap();
    let files = env_files_of("shop", "web").unwrap();
    assert_eq!(files["dev"].len(), 1, "{files:?}");
    assert_eq!(files["dev"]["orbstack"], ".env.orb");
}

#[test]
fn contexts_are_validated_and_canonicalised() {
    let dir = TempConfigDir::new();
    two_apis(&dir);

    add_env_file("web", None, "dev", "compose", ".env.compose").unwrap();
    assert_eq!(
        env_files_of("shop", "web").unwrap()["dev"]["docker-compose"],
        ".env.compose"
    );

    let err = add_env_file("web", None, "dev", "dokcer", ".env")
        .unwrap_err()
        .to_string();
    assert!(err.contains("Did you mean 'docker'?"), "{err}");

    assert!(set_default_stage("web", None, "nope", "dev").is_err());
    set_default_stage("web", None, "kubernetes", "prod").unwrap();
    let config = load_config().unwrap();
    assert_eq!(
        config.projects["shop"].apps["web"]
            .default_stages
            .as_ref()
            .unwrap()["k8s"],
        "prod"
    );

    // An alias removes the canonical entry; a context that is not there is an error.
    assert!(remove_env_file("web", None, "dev", Some("local")).is_err());
    remove_env_file("web", None, "dev", Some("compose")).unwrap();
    assert!(env_files_of("shop", "web").is_none());
}
