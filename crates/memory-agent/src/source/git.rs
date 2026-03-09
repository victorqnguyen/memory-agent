use std::path::Path;

use git2::{Oid, Repository};

pub struct GitContext {
    repo: Repository,
}

impl GitContext {
    pub fn open(dir: &Path) -> Option<Self> {
        Repository::discover(dir).ok().map(|repo| Self { repo })
    }

    pub fn changed_files(&self, since_commit: &str) -> anyhow::Result<Vec<String>> {
        let old = self.repo.find_commit(Oid::from_str(since_commit)?)?;
        let head = self.repo.head()?.peel_to_commit()?;

        if old.id() == head.id() {
            return Ok(Vec::new());
        }

        let diff = self.repo.diff_tree_to_tree(
            Some(&old.tree()?),
            Some(&head.tree()?),
            None,
        )?;

        let mut files = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path().and_then(|p| p.to_str()) {
                    files.push(path.to_string());
                }
                true
            },
            None,
            None,
            None,
        )?;

        Ok(files)
    }
}
