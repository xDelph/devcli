//! Kubernetes environment capability detection strategy

use crate::{context::DetectionContext, strategy::DetectionStrategy, types::*, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Detects Kubernetes environment via manifest files
#[derive(Default)]
pub struct KubernetesEnvStrategy;

/// Directories whose YAML files are applied as a whole.
const MANIFEST_DIRS: &[&str] = &["k8s", "kubernetes", ".kube"];

/// File names `kubectl apply -k` reads.
const KUSTOMIZATION_FILES: &[&str] = &["kustomization.yaml", "kustomization.yml", "Kustomization"];

fn is_kustomization_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| KUSTOMIZATION_FILES.contains(&n))
}

fn has_kustomization(dir: &Path) -> bool {
    KUSTOMIZATION_FILES
        .iter()
        .any(|name| dir.join(name).is_file())
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == "yaml" || e == "yml")
}

/// A Helm chart: its directory (relative to the root) and release name.
struct HelmChart {
    dir: String,
    release: String,
}

/// The first Helm chart found: at the root, in `chart/` or `helm/`, or one
/// level below `charts/` or `helm/` (sorted, so the pick is stable).
fn find_helm_chart(root: &Path) -> Option<HelmChart> {
    let mut candidates = vec![PathBuf::from("."), "chart".into(), "helm".into()];
    for parent in ["charts", "helm"] {
        if let Ok(entries) = std::fs::read_dir(root.join(parent)) {
            let mut dirs: Vec<PathBuf> = entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| Path::new(parent).join(e.file_name()))
                .collect();
            dirs.sort();
            candidates.extend(dirs);
        }
    }
    candidates.into_iter().find_map(|dir| {
        let content = std::fs::read_to_string(root.join(&dir).join("Chart.yaml")).ok()?;
        let folder = if dir == Path::new(".") {
            root.canonicalize().ok()?.file_name()?.to_str()?.to_string()
        } else {
            dir.file_name()?.to_str()?.to_string()
        };
        let release = release_name(chart_name(&content).as_deref().unwrap_or(&folder));
        Some(HelmChart {
            dir: dir.to_string_lossy().replace('\\', "/"),
            release,
        })
    })
}

/// The top-level `name:` of a Chart.yaml.
fn chart_name(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let value = line.strip_prefix("name:")?;
        let value = value.split('#').next()?.trim().trim_matches(['"', '\'']);
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// A Helm release name: lowercase letters, digits and `-`.
fn release_name(raw: &str) -> String {
    let name: String = raw
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let name = name.trim_matches('-');
    if name.is_empty() {
        "app".to_string()
    } else {
        name.to_string()
    }
}

impl DetectionStrategy for KubernetesEnvStrategy {
    fn id(&self) -> &str {
        "kubernetes-env"
    }

    fn name(&self) -> &str {
        "Kubernetes"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::EnvCapability(EnvCapabilityCategory::Kubernetes)
    }

    fn priority(&self) -> usize {
        450 // Between Docker (400) and Local (500)
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Specific k8s directory structures (most reliable signal)
        MANIFEST_DIRS.iter().any(|dir| ctx.file_exists(dir))
            // Kustomize and Helm are k8s-specific
            || KUSTOMIZATION_FILES.iter().any(|name| ctx.file_exists(name))
            || find_helm_chart(&ctx.root_path).is_some()
            // Well-known k8s manifest filenames at root
            || ctx.file_exists("deployment.yaml")
            || ctx.file_exists("deployment.yml")
            || ctx.file_exists("service.yaml")
            || ctx.file_exists("service.yml")
            || ctx.file_exists("ingress.yaml")
            || ctx.file_exists("ingress.yml")
            // Suffix-style manifests (e.g. deployment.k8s.yaml)
            || !ctx.glob("*.k8s.yaml").is_empty()
            || !ctx.glob("*.k8s.yml").is_empty()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut manifests = Vec::new();
        let mut helm_charts = Vec::new();
        let mut commands = HashMap::new();
        let mut metadata = HashMap::new();

        // Helm: `helm install` / `helm upgrade` need a release name and a
        // chart, so they are only generated for a chart that was found.
        let chart = find_helm_chart(&ctx.root_path);
        if let Some(chart) = &chart {
            helm_charts.push(Path::new(&chart.dir).join("Chart.yaml"));
            metadata.insert("has_helm".to_string(), serde_json::json!(true));
            let HelmChart { dir, release } = chart;
            commands.insert(
                "install".to_string(),
                format!("helm upgrade --install {release} {dir}"),
            );
            commands.insert(
                "upgrade".to_string(),
                format!("helm upgrade {release} {dir}"),
            );
            commands.insert("uninstall".to_string(), format!("helm uninstall {release}"));
        }

        // What `apply` / `delete` target: (`-k` or `-f`, directory).
        // A directory with a kustomization is always applied with `-k`:
        // `-f` sends the kustomization file itself to the API server, and
        // `kubectl apply -f .` on a kustomize directory fails.
        let mut apply_target: Option<(&str, String)> = None;

        if has_kustomization(&ctx.root_path) {
            metadata.insert("has_kustomize".to_string(), serde_json::json!(true));
            apply_target = Some(("-k", ".".to_string()));
        }

        for k8s_dir in MANIFEST_DIRS {
            let k8s_path = ctx.root_path.join(k8s_dir);
            if !k8s_path.is_dir() {
                continue;
            }

            let mut dir_manifests: Vec<PathBuf> = std::fs::read_dir(&k8s_path)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|path| {
                            path.is_file() && is_yaml(path) && !is_kustomization_file(path)
                        })
                        .filter_map(|path| {
                            path.strip_prefix(&ctx.root_path)
                                .ok()
                                .map(Path::to_path_buf)
                        })
                        .collect()
                })
                .unwrap_or_default();
            dir_manifests.sort();

            if apply_target.is_none() {
                if has_kustomization(&k8s_path) {
                    metadata.insert("has_kustomize".to_string(), serde_json::json!(true));
                    apply_target = Some(("-k", format!("{k8s_dir}/")));
                } else if !dir_manifests.is_empty() {
                    apply_target = Some(("-f", format!("{k8s_dir}/")));
                }
            }
            manifests.extend(dir_manifests);
        }

        // Kubernetes manifests in the root (content has a `kind:` field). A
        // chart's own Chart.yaml / values.yaml are not manifests.
        if chart.as_ref().is_none_or(|c| c.dir != ".") {
            let mut root_manifests: Vec<PathBuf> = ctx
                .glob("*.yaml")
                .into_iter()
                .chain(ctx.glob("*.yml"))
                .filter(|path| !is_kustomization_file(path) && is_kubernetes_manifest(ctx, path))
                .collect();
            root_manifests.sort();
            if apply_target.is_none() && !root_manifests.is_empty() {
                apply_target = Some(("-f", ".".to_string()));
            }
            manifests.extend(root_manifests);
        }

        if let Some((flag, dir)) = &apply_target {
            commands.insert("apply".to_string(), format!("kubectl apply {flag} {dir}"));
            commands.insert("delete".to_string(), format!("kubectl delete {flag} {dir}"));
        }

        if !commands.is_empty() {
            commands
                .entry("get".to_string())
                .or_insert_with(|| "kubectl get all".to_string());
            commands
                .entry("logs".to_string())
                .or_insert_with(|| "kubectl logs".to_string());
            commands
                .entry("restart".to_string())
                .or_insert_with(|| "kubectl rollout restart deployment".to_string());
        }

        metadata.insert(
            "manifest_count".to_string(),
            serde_json::json!(manifests.len()),
        );
        metadata.insert(
            "helm_chart_count".to_string(),
            serde_json::json!(helm_charts.len()),
        );

        // Suggest default command
        let suggested_default = if commands.contains_key("apply") {
            Some("apply".to_string())
        } else if commands.contains_key("install") {
            Some("install".to_string())
        } else {
            None
        };

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: if manifests.is_empty() && helm_charts.is_empty() {
                0.5 // Lower confidence if no explicit K8s files found
            } else {
                1.0
            },
            data: DetectionData::KubernetesEnv(KubernetesEnvInfo {
                manifests,
                helm_charts,
                commands,
                suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }
}

/// Check if a YAML file is a Kubernetes manifest by looking for "kind:" field
fn is_kubernetes_manifest(ctx: &DetectionContext, path: &PathBuf) -> bool {
    if let Ok(content) = ctx.read_file(path) {
        let content_lower = content.to_lowercase();
        // Look for common Kubernetes kinds
        content_lower.contains("kind:")
            && (content_lower.contains("deployment")
                || content_lower.contains("service")
                || content_lower.contains("pod")
                || content_lower.contains("configmap")
                || content_lower.contains("secret")
                || content_lower.contains("ingress")
                || content_lower.contains("statefulset")
                || content_lower.contains("daemonset")
                || content_lower.contains("job")
                || content_lower.contains("cronjob"))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_kubernetes_with_manifests() {
        let temp_dir = TempDir::new().unwrap();

        // Create k8s directory with manifests
        fs::create_dir(temp_dir.path().join("k8s")).unwrap();

        let deployment = temp_dir.path().join("k8s/deployment.yaml");
        let mut file = fs::File::create(&deployment).unwrap();
        file.write_all(b"apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: my-app\n")
            .unwrap();

        let service = temp_dir.path().join("k8s/service.yaml");
        let mut file = fs::File::create(&service).unwrap();
        file.write_all(b"apiVersion: v1\nkind: Service\nmetadata:\n  name: my-app\n")
            .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = KubernetesEnvStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "kubernetes-env");
        assert_eq!(result.confidence, 1.0);

        match result.data {
            DetectionData::KubernetesEnv(info) => {
                assert!(
                    info.manifests.len() >= 2,
                    "Expected at least 2 manifests, got {}",
                    info.manifests.len()
                );
                assert!(info.commands.contains_key("apply"));
                assert_eq!(info.suggested_default, Some("apply".to_string()));
            }
            _ => panic!("Expected KubernetesEnv data"),
        }
    }

    #[test]
    fn test_kubernetes_with_helm() {
        let temp_dir = TempDir::new().unwrap();

        // Create Helm chart
        let chart = temp_dir.path().join("Chart.yaml");
        let mut file = fs::File::create(&chart).unwrap();
        file.write_all(b"apiVersion: v2\nname: my-chart\nversion: 1.0.0\n")
            .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = KubernetesEnvStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::KubernetesEnv(info) => {
                assert_eq!(info.helm_charts.len(), 1);
                assert!(info.commands.contains_key("install"));
                assert!(info.metadata.contains_key("has_helm"));
            }
            _ => panic!("Expected KubernetesEnv data"),
        }
    }

    #[test]
    fn test_kubernetes_with_kustomize() {
        let temp_dir = TempDir::new().unwrap();

        // Create kustomization.yaml
        let kustomize = temp_dir.path().join("kustomization.yaml");
        let mut file = fs::File::create(&kustomize).unwrap();
        file.write_all(b"resources:\n  - deployment.yaml\n  - service.yaml\n")
            .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = KubernetesEnvStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::KubernetesEnv(info) => {
                assert!(info.commands.contains_key("apply"));
                assert!(info.metadata.contains_key("has_kustomize"));
            }
            _ => panic!("Expected KubernetesEnv data"),
        }
    }

    #[test]
    fn test_kubernetes_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = KubernetesEnvStrategy;

        // Should not apply without K8s markers
        assert!(!strategy.can_apply(&ctx));
    }

    fn k8s_commands(dir: &Path) -> HashMap<String, String> {
        let ctx = DetectionContext::new(dir).unwrap();
        assert!(KubernetesEnvStrategy.can_apply(&ctx));
        match KubernetesEnvStrategy.detect(&ctx).unwrap().data {
            DetectionData::KubernetesEnv(info) => info.commands,
            _ => panic!("Expected KubernetesEnv data"),
        }
    }

    const DEPLOYMENT: &str = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n";

    #[test]
    fn root_kustomization_is_applied_with_k_even_next_to_manifests() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("kustomization.yaml"),
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - deployment.yaml\n  - service.yaml\n",
        )
        .unwrap();
        fs::write(temp_dir.path().join("deployment.yaml"), DEPLOYMENT).unwrap();

        let commands = k8s_commands(temp_dir.path());
        assert_eq!(commands["apply"], "kubectl apply -k .");
        assert_eq!(commands["delete"], "kubectl delete -k .");
    }

    #[test]
    fn kustomize_under_k8s_is_applied_with_k() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("k8s")).unwrap();
        fs::write(
            temp_dir.path().join("k8s/kustomization.yml"),
            "kind: Kustomization\nresources:\n  - deployment.yaml\n",
        )
        .unwrap();
        fs::write(temp_dir.path().join("k8s/deployment.yaml"), DEPLOYMENT).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let DetectionData::KubernetesEnv(info) = KubernetesEnvStrategy.detect(&ctx).unwrap().data
        else {
            panic!("Expected KubernetesEnv data");
        };
        assert_eq!(info.commands["apply"], "kubectl apply -k k8s/");
        assert_eq!(
            info.manifests,
            vec![PathBuf::from("k8s/deployment.yaml")],
            "the kustomization file is not a manifest"
        );
    }

    #[test]
    fn plain_manifest_directories_are_applied_with_f() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("kubernetes")).unwrap();
        fs::write(temp_dir.path().join("kubernetes/web.yaml"), DEPLOYMENT).unwrap();

        let commands = k8s_commands(temp_dir.path());
        assert_eq!(commands["apply"], "kubectl apply -f kubernetes/");
        assert_eq!(commands["delete"], "kubectl delete -f kubernetes/");
    }

    #[test]
    fn helm_commands_name_a_release_and_a_chart() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("Chart.yaml"),
            "apiVersion: v2\nname: My_Chart # the chart\nversion: 1.0.0\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("values.yaml"),
            "service:\n  kind: ClusterIP\n",
        )
        .unwrap();

        let commands = k8s_commands(temp_dir.path());
        assert_eq!(commands["install"], "helm upgrade --install my-chart .");
        assert_eq!(commands["upgrade"], "helm upgrade my-chart .");
        assert_eq!(commands["uninstall"], "helm uninstall my-chart");
        assert!(
            !commands.contains_key("apply"),
            "values.yaml is not a manifest: {commands:?}"
        );
    }

    #[test]
    fn helm_chart_in_a_subdirectory_is_found() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("charts/api")).unwrap();
        fs::write(
            temp_dir.path().join("charts/api/Chart.yaml"),
            "apiVersion: v2\nname: api\nversion: 0.1.0\n",
        )
        .unwrap();

        let commands = k8s_commands(temp_dir.path());
        assert_eq!(commands["install"], "helm upgrade --install api charts/api");
    }

    #[test]
    fn no_helm_commands_without_a_chart() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("deployment.yaml"), DEPLOYMENT).unwrap();

        let commands = k8s_commands(temp_dir.path());
        assert_eq!(commands["apply"], "kubectl apply -f .");
        assert!(!commands.contains_key("install"));
        assert!(!commands.contains_key("upgrade"));
    }
}
