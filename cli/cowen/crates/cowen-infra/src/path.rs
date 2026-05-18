use std::path::PathBuf;
use directories;

pub fn get_app_dir() -> PathBuf {
    if let Ok(path) = std::env::var("COWEN_HOME") {
        let p = PathBuf::from(path);
        if p.is_relative() {
            if let Ok(cwd) = std::env::current_dir() {
                return cwd.join(p);
            }
        }
        return p;
    }
    let home = directories::BaseDirs::new()
        .unwrap()
        .home_dir()
        .to_path_buf();
    home.join(".cowen")
}
