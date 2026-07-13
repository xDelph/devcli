//! Kubernetes environment capability detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Detects Kubernetes environment via manifest files
#[derive(Default)]
pub struct KubernetesEnvStrategy;

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
        ctx.file_exists("k8s")
            || ctx.file_exists("kubernetes")
            || ctx.file_exists(".kube")
            // Kustomize and Helm are k8s-specific
            || ctx.file_exists("kustomization.yaml")
            || ctx.file_exists("kustomization.yml")
            || ctx.file_exists("Chart.yaml")
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

        // Look for Helm charts
        if ctx.file_exists("Chart.yaml") {
            helm_charts.push(PathBuf::from("Chart.yaml"));
            metadata.insert("has_helm".to_string(), serde_json::json!(true));

            // Add Helm commands
            commands.insert("install".to_string(), "helm install".to_string());
            commands.insert("upgrade".to_string(), "helm upgrade".to_string());
            commands.insert("uninstall".to_string(), "helm uninstall".to_string());
        }

        // Look for kustomization
        if ctx.file_exists("kustomization.yaml") || ctx.file_exists("kustomization.yml") {
            metadata.insert("has_kustomize".to_string(), serde_json::json!(true));
            commands.insert(
                "apply".to_string(),
                "kubectl apply -k .".to_string(),
            );
        }

        // Look for k8s/ or kubernetes/ directories
        for k8s_dir in &["k8s", "kubernetes", ".kube"] {
            if ctx.file_exists(k8s_dir) {
                let k8s_path = ctx.root_path.join(k8s_dir);

                // Find all YAML files in k8s directory
                if let Ok(entries) = std::fs::read_dir(&k8s_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap().to_string_lossy();
                            if file_name.ends_with(".yaml") || file_name.ends_with(".yml") {
                                if let Ok(rel_path) = path.strip_prefix(&ctx.root_path) {
                                    manifests.push(rel_path.to_path_buf());
                                }
                            }
                        }
                    }
                }

                // Add kubectl apply command for this directory
                if !commands.contains_key("apply") {
                    commands.insert(
                        "apply".to_string(),
                        format!("kubectl apply -f {}", k8s_dir),
                    );
                }
            }
        }

        // Scan for Kubernetes manifests in root (check content for kind: field)
        for yaml_file in ctx.glob("*.yaml").iter().chain(ctx.glob("*.yml").iter()) {
            if is_kubernetes_manifest(ctx, yaml_file) {
                manifests.push(yaml_file.clone());
            }
        }

        // Add standard kubectl commands if we found manifests
        if !manifests.is_empty() && !commands.contains_key("apply") {
            commands.insert("apply".to_string(), "kubectl apply -f .".to_string());
        }

        if !commands.is_empty() {
            commands.insert("get".to_string(), "kubectl get all".to_string());
            commands.insert("delete".to_string(), "kubectl delete -f .".to_string());
            commands.insert("logs".to_string(), "kubectl logs".to_string());
        }

        metadata.insert("manifest_count".to_string(), serde_json::json!(manifests.len()));
        metadata.insert("helm_chart_count".to_string(), serde_json::json!(helm_charts.len()));

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
        file.write_all(
            b"apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: my-app\n",
        )
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
                assert!(info.manifests.len() >= 2, "Expected at least 2 manifests, got {}", info.manifests.len());
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
}
