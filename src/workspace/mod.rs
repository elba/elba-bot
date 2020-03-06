mod index;
mod repo;
mod store;

pub use self::index::Index;
pub use self::repo::Repo;
pub use self::store::Store;

use elba::package::manifest::Manifest;

use crate::config::CONFIG;
use crate::error::Result;

pub struct Workspace {
    pub index: Index,
    pub store: Store,
}

impl Workspace {
    pub fn new() -> Result<Self> {
        Ok(Workspace {
            index: Index::clone()?,
            store: Store::clone()?,
        })
    }
}

fn tarball_name(manifest: &Manifest) -> String {
    format!(
        "{}_{}_{}.tar.gz",
        &manifest.package.name.normalized_group(),
        &manifest.package.name.normalized_name(),
        &manifest.package.version
    )
}

fn github_raw_url(head_hash: &str, manifest: &Manifest) -> String {
    format!(
        "https://github.com/{}/blob/{}/{}/{}/{}?raw=true",
        &CONFIG.store_repo_name,
        head_hash,
        &manifest.package.name.normalized_group(),
        &manifest.package.name.normalized_name(),
        &tarball_name(manifest)
    )
}

fn github_repo_url(repo_name: &str) -> String {
    format!("https://github.com/{}.git", repo_name)
}
