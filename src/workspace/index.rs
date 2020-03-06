use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use elba::package::manifest::{DepReq, Manifest};
use elba::remote::{resolution::DirectRes, RawDep, RawEntry};
use failure::bail;
use itertools::Itertools;
use log::info;

use super::Repo;
use super::*;
use crate::config::CONFIG;

use crate::error::{Error, Result};

pub struct Index {
    repo: Repo,
}

impl Index {
    pub fn clone() -> Result<Self> {
        Ok(Index {
            repo: Repo::clone(
                &github_repo_url(&CONFIG.index_repo_name),
                &CONFIG.index_checkout,
            )?,
        })
    }

    pub fn update_package(&self, manifest: &Manifest, location: &DirectRes) -> Result<()> {
        info!(
            "Updating index entries to publish `{} {}`",
            &manifest.package.name, &manifest.package.version
        );

        self.repo.fetch_and_reset()?;

        // Update metafile entry
        let name = &manifest.package.name;
        let metafile_path = self
            .repo
            .workdir()?
            .join(name.normalized_group())
            .join(name.normalized_name());
        let mut entries = if metafile_path.exists() {
            Entries::load(&metafile_path)?
        } else {
            Entries::empty()
        };
        entries.insert(manifest, location)?;
        entries.save(&metafile_path)?;

        self.repo.commit_and_push(
            &format!(
                "Update Package `{} {}`",
                &manifest.package.name, &manifest.package.version
            ),
            &metafile_path,
        )?;

        info!(
            "Updated index entries to publish `{} {}`",
            &manifest.package.name, &manifest.package.version
        );

        Ok(())
    }

    pub fn update_readme(&self, package_list: String) -> Result<()> {
        info!("Updating index readme");

        self.repo.fetch_and_reset()?;

        let readme_path = self.repo.workdir()?.join("README.md");
        let readme_template_path = self.repo.workdir()?.join("README.TEMPLATE");
        let mut readme = OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open(&readme_path)?;
        let mut readme_template = File::open(readme_template_path)?;

        // Read the README template and replace the package list placeholder
        let mut content = String::new();
        readme_template.read_to_string(&mut content)?;
        content = content.replace("{#package-list#}", &package_list);
        readme.write_all(content.as_bytes())?;
        readme.sync_all()?;

        self.repo.commit_and_push(&"Update README", &readme_path)?;

        info!("Updated index readme");

        Ok(())
    }
}

pub struct Entries(Vec<RawEntry>);

impl Entries {
    pub fn empty() -> Self {
        Entries(Vec::new())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let entries: Vec<RawEntry> = content
            .split("\n")
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        Ok(Entries(entries))
    }

    fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        let mut file = OpenOptions::new()
            .truncate(true)
            .create(true)
            .write(true)
            .open(&path)?;
        let content = self
            .0
            .iter()
            .filter_map(|entry| serde_json::to_string(entry).ok())
            .join("\n");
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    pub fn insert(&mut self, manifest: &Manifest, location: &DirectRes) -> Result<()> {
        let mut dependencies = Vec::new();
        for (name, req) in manifest.dependencies.iter() {
            let req = match req {
                DepReq::Registry(constrain) => constrain.clone(),
                _ => bail!(Error::NonIndexDependency {
                    dependency: name.to_string(),
                    resolution: format!("{:?}", req)
                }),
            };
            dependencies.push(RawDep {
                name: name.clone(),
                req,
                index: None,
            });
        }

        let entry = RawEntry {
            name: manifest.package.name.clone(),
            version: manifest.package.version.clone(),
            location: Some(location.clone()),
            dependencies,
            yanked: false,
        };

        // fix potential violation
        self.0
            .retain(|other| other.name != entry.name || other.version != entry.version);

        self.0.push(entry);

        Ok(())
    }
}
