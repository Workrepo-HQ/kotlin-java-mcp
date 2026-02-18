pub mod parser;

use std::path::{Path, PathBuf};
use std::process::Command;

use parking_lot::RwLock;
use tracing::debug;

use crate::error::GradleError;

#[derive(Debug, Clone, Default)]
pub struct GradleInfo {
    pub modules: Vec<GradleModule>,
    pub root_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GradleModule {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub resolved_version: Option<String>,
    pub is_project: bool,
    pub is_transitive_duplicate: bool,
    pub children: Vec<DependencyNode>,
}

pub struct GradleRunner {
    project_root: PathBuf,
    cached_info: RwLock<Option<GradleInfo>>,
}

impl GradleRunner {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            cached_info: RwLock::new(None),
        }
    }

    fn gradlew_path(&self) -> PathBuf {
        self.project_root.join("gradlew")
    }

    fn has_gradlew(&self) -> bool {
        self.gradlew_path().exists()
    }

    pub fn invalidate_cache(&self) {
        *self.cached_info.write() = None;
    }

    pub fn get_modules(&self) -> Result<Vec<GradleModule>, GradleError> {
        // Check cache
        if let Some(ref info) = *self.cached_info.read() {
            return Ok(info.modules.clone());
        }

        if !self.has_gradlew() {
            return Err(GradleError::WrapperNotFound(
                self.gradlew_path().display().to_string(),
            ));
        }

        let output = Command::new(self.gradlew_path())
            .arg("projects")
            .arg("-q")
            .current_dir(&self.project_root)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GradleError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let modules = parser::parse_projects_output(&stdout);

        debug!("Found {} Gradle modules", modules.len());

        // Update cache
        let mut cache = self.cached_info.write();
        let info = cache.get_or_insert_with(|| GradleInfo {
            modules: Vec::new(),
            root_path: self.project_root.clone(),
        });
        info.modules = modules.clone();

        Ok(modules)
    }

    pub fn get_dependencies(
        &self,
        module: &str,
    ) -> Result<Vec<DependencyNode>, GradleError> {
        if !self.has_gradlew() {
            return Err(GradleError::WrapperNotFound(
                self.gradlew_path().display().to_string(),
            ));
        }

        let module_arg = if module.starts_with(':') {
            format!("{}:dependencies", module)
        } else {
            format!(":{}:dependencies", module)
        };

        let output = Command::new(self.gradlew_path())
            .arg(&module_arg)
            .arg("--configuration")
            .arg("compileClasspath")
            .arg("-q")
            .current_dir(&self.project_root)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GradleError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let deps = parser::parse_dependencies_output(&stdout);

        Ok(deps)
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}
