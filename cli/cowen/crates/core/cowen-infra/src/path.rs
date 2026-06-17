use directories;
use std::path::PathBuf;

pub fn get_app_dir() -> PathBuf {
    let env_home = std::env::var("COWEN_HOME").ok();
    let home_dir = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf());
    get_app_dir_internal(env_home, home_dir)
}

fn is_dir_writable(path: &std::path::Path) -> bool {
    if !path.exists() && std::fs::create_dir_all(path).is_err() {
        return false;
    }
    let test_file = path.join(format!(".write_test_{}", std::process::id()));
    match std::fs::write(&test_file, b"") {
        Ok(_) => {
            let _ = std::fs::remove_file(test_file);
            true
        }
        Err(_) => false,
    }
}

fn get_app_dir_internal(env_cowen_home: Option<String>, home_dir: Option<PathBuf>) -> PathBuf {
    if let Some(path) = env_cowen_home {
        let p = PathBuf::from(path);
        if p.is_relative() {
            if let Ok(cwd) = std::env::current_dir() {
                return cwd.join(p);
            }
        }
        return p;
    }

    let default_path = if let Some(hd) = home_dir {
        hd.join(".cowen")
    } else {
        std::env::temp_dir().join(".cowen")
    };

    if is_dir_writable(&default_path) {
        default_path
    } else {
        let temp_path = std::env::temp_dir().join(".cowen");
        tracing::warn!(
            "Directory '{}' is not writable. Falling back to temporary directory '{}'.",
            default_path.display(),
            temp_path.display()
        );
        temp_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_get_app_dir_internal_env_cowen_home() {
        // Absolute COWEN_HOME
        let res = get_app_dir_internal(Some("/tmp/custom_cowen".to_string()), None);
        assert_eq!(res, PathBuf::from("/tmp/custom_cowen"));

        // Relative COWEN_HOME
        let res = get_app_dir_internal(Some(".custom_cowen".to_string()), None);
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(res, cwd.join(".custom_cowen"));
    }

    #[test]
    fn test_get_app_dir_internal_default_writable() {
        let dir = tempdir().unwrap();
        let home_path = dir.path().to_path_buf();

        let res = get_app_dir_internal(None, Some(home_path.clone()));
        assert_eq!(res, home_path.join(".cowen"));
    }

    #[test]
    fn test_get_app_dir_internal_default_not_writable() {
        // A path that is guaranteed not to be writable (e.g. root level nonexistent path)
        let unwritable_home = PathBuf::from("/nonexistent_forbidden_dir_path_xxx");

        let res = get_app_dir_internal(None, Some(unwritable_home));
        // It should fallback to standard temp dir
        let expected_fallback = std::env::temp_dir().join(".cowen");
        assert_eq!(res, expected_fallback);
    }
}
