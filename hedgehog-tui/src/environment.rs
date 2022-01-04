use std::{
    io,
    path::{Path, PathBuf},
};

pub(crate) struct AppEnvironment {
    pub(crate) data_path: PathBuf,
    pub(crate) config_path: Vec<PathBuf>,
}

impl AppEnvironment {
    pub(crate) fn new_with_data_path(data_path: PathBuf) -> Self {
        AppEnvironment {
            data_path,
            config_path: Vec::new(),
        }
    }

    pub(crate) fn push_config_path(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let abs_path = match std::env::current_dir() {
            Ok(mut current_path) => {
                current_path.push(path);
                current_path.canonicalize()?
            }
            Err(_) => path.as_ref().canonicalize()?,
        };

        for (index, path) in self.config_path.iter().enumerate() {
            if path == &abs_path {
                self.config_path.remove(index);
                break;
            }
        }
        self.config_path.push(abs_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppEnvironment;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn push_config_path() {
        let mut env = AppEnvironment::new_with_data_path(Path::new("/data").to_path_buf());

        let tmp_path = tempdir().unwrap();

        let mut path = tmp_path.path().to_path_buf();
        path.push("first");
        std::fs::create_dir(&path).unwrap();
        let path1 = path.clone();
        path.pop();

        path.push("second");
        std::fs::create_dir(&path).unwrap();
        let path2 = path.clone();

        path.pop();
        path.push("first");
        path.push(".");
        path.push("..");
        path.push("first");
        let path3 = path;

        env.push_config_path(&path1).unwrap();
        assert!(&env.config_path.iter().eq(vec![&path1].into_iter()));

        env.push_config_path(&path2).unwrap();
        assert!(&env.config_path.iter().eq(vec![&path1, &path2].into_iter()));

        env.push_config_path(&path3).unwrap();
        assert!(&env.config_path.iter().eq(vec![&path2, &path1].into_iter()));
    }
}
