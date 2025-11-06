#[cfg(test)]
mod tests {
    use crate::detection::utils::{find_dockerfile, find_k8s_files};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_find_dockerfile_in_root() {
        let temp_dir = create_test_dir();
        let dockerfile = temp_dir.path().join("Dockerfile");
        fs::write(&dockerfile, "FROM node:16").unwrap();

        let result = find_dockerfile(temp_dir.path()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), dockerfile);
    }

    #[test]
    fn test_find_dockerfile_not_found() {
        let temp_dir = create_test_dir();
        let random_file = temp_dir.path().join("random.txt");
        fs::write(&random_file, "not a dockerfile").unwrap();

        let result = find_dockerfile(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_k8s_files_in_k8s_directory() {
        let temp_dir = create_test_dir();
        let k8s_dir = temp_dir.path().join("k8s");
        fs::create_dir(&k8s_dir).unwrap();
        
        let deployment = k8s_dir.join("deployment.yaml");
        let service = k8s_dir.join("service.yml");
        fs::write(&deployment, "apiVersion: apps/v1").unwrap();
        fs::write(&service, "apiVersion: v1").unwrap();

        let result = find_k8s_files(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"k8s/deployment.yaml".to_string()));
        assert!(result.contains(&"k8s/service.yml".to_string()));
    }

    #[test]
    fn test_find_k8s_files_empty_directory() {
        let temp_dir = create_test_dir();
        let random_file = temp_dir.path().join("random.txt");
        fs::write(&random_file, "not k8s related").unwrap();

        let result = find_k8s_files(temp_dir.path()).unwrap();
        assert!(result.is_empty());
    }
}