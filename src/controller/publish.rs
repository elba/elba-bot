use std::fmt::Write;

use elba::package::{manifest::Manifest, Name as PackageName};
use failure::bail;
use semver::Version;
use tokio::task::block_in_place;

use super::*;
use crate::config::CONFIG;
use crate::database::{self};
use crate::error::{Error, Result};
use crate::github::{self, Comment};
use crate::workspace::Repo;

impl Controller {
    pub async fn publish(
        &self,
        remote_url: String,
        refname: Option<String>,
        comment: Comment,
    ) -> Result<()> {
        let mut state = PublishState {
            step: PublishStep::Block,
            remote_url: remote_url.clone(),
            name: None,
            error: None,
        };

        let res: Result<()> = try {
            self.update_report(&comment, &state).await?;

            let workspace = self.workspace.lock().await;

            // Pull remote repository
            state.step = PublishStep::Pull;
            self.update_report(&comment, &state).await?;
            let pull_dir = tempdir::TempDir::new(&CONFIG.bot_name)?;
            let pull_repo = block_in_place(|| Repo::clone(&remote_url, pull_dir.as_ref()))?;
            if let Some(refname) = refname {
                pull_repo.checkout(&refname)?;
            }

            // Build package tarball and check manifest
            state.step = PublishStep::Verify;
            self.update_report(&comment, &state).await?;
            let (tarball, manifest) =
                block_in_place(|| elba::cli::index::package(pull_repo.workdir()?))?;

            self.check_publish_permission(&manifest, &comment.user)
                .await?;
            state.name = Some((
                manifest.package.name.clone(),
                manifest.package.version.clone(),
            ));

            // Upload talball to store repository
            state.step = PublishStep::Upload;
            self.update_report(&comment, &state).await?;
            let location = block_in_place(|| workspace.store.upload_package(&manifest, &tarball))?;

            // Update index entry and commit the metadata into database, then update readme
            state.step = PublishStep::UpdateIndex;
            block_in_place(|| workspace.index.update_package(&manifest, &location))?;
            self.commit_publish(&manifest, &comment.user).await?;
            let package_list = render_readme_package_list(&*self.database.lock().await)?;
            block_in_place(|| workspace.index.update_readme(package_list))?;

            ()
        };

        match res {
            Ok(()) => {
                state.step = PublishStep::Done;
                self.update_report(&comment, &state).await?;
                info!("Publish done: {:?}", state);
            }
            Err(error) => {
                state.error = Some(error.to_string());
                self.update_report(&comment, &state).await?;
                info!("Publish error: {:?}", state);
            }
        }

        Ok(())
    }

    /// Query database and check whether the user has permission to publish
    async fn check_publish_permission(
        &self,
        manifest: &Manifest,
        user: &github::User,
    ) -> Result<()> {
        let database = self.database.lock().await;
        let packages_in_group =
            database.query_package(Some(manifest.package.name.normalized_group()))?;

        // Check whether the user owns the namespace
        let conflict_package = packages_in_group
            .iter()
            .filter(|package| package.user_id != user.id)
            .next();
        if let Some(conflict_package) = conflict_package {
            let namespace_owner = database.query_user(conflict_package.user_id)?.unwrap();
            bail!(Error::NamespaceIsTaken {
                group: conflict_package.group.to_string(),
                owner: namespace_owner.name
            });
        };

        // Check whether the package exists
        let exist_same_package = packages_in_group.iter().any(|package| {
            package.name == manifest.package.name.normalized_name()
                && package.version == manifest.package.version
        });
        if exist_same_package {
            bail!(Error::PackageExists {
                package: manifest.package.name.to_string(),
                version: manifest.package.version.clone(),
            });
        }

        Ok(())
    }

    /// Commit package metadata to database
    async fn commit_publish(&self, manifest: &Manifest, user: &github::User) -> Result<()> {
        let database = self.database.lock().await;
        database.insert_user(database::User {
            id: user.id,
            name: user.name.clone(),
        })?;
        database.insert_package(database::Package {
            group: manifest.package.name.normalized_group().to_string(),
            name: manifest.package.name.normalized_name().to_string(),
            version: manifest.package.version.clone(),
            description: manifest.package.description.clone(),
            user_id: user.id,
        })?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct PublishState {
    pub step: PublishStep,
    pub remote_url: String,
    pub name: Option<(PackageName, Version)>,
    pub error: Option<String>,
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum PublishStep {
    Block,
    Pull,
    Verify,
    Upload,
    UpdateIndex,
    Done,
}

impl CommentReport for PublishState {
    fn render_title(&self, _: &Comment) -> Option<&str> {
        Some("Publish Package")
    }

    fn render_body(&self, _: &Comment) -> Option<String> {
        let mut body = String::new();

        if self.step == PublishStep::Block {
            body += "- 🎅 Blocking waiting for previous tasks\n";
        } else {
            if self.step >= PublishStep::Pull {
                body += "- 🚢 Pulling repository\n";
            }
            if self.step >= PublishStep::Verify {
                body += "- 🏭 Verifying package\n";
            }
            if self.step >= PublishStep::Upload {
                body += "- 📦 Uploading package\n";
            }
            if self.step >= PublishStep::UpdateIndex {
                body += "- 📜 Updating index\n";
            }
            if self.step >= PublishStep::Done {
                body += "- ✔️ Done\n";
            }
        }

        if let Some(error) = &self.error {
            write!(body, "  - ❌ *{}*\n\n", error).unwrap();
        }

        Some(body)
    }

    fn render_msg(&self, _: &Comment) -> String {
        if let Some(_) = &self.error {
            "Publish failed due to the reason above.".to_owned()
        } else {
            match self.step {
                PublishStep::Block => "Publish process will be started soon.".to_owned(),
                PublishStep::Done => format!(
                    "Package  `{}|{}` has been published. 🚀",
                    self.name.as_ref().unwrap().0,
                    self.name.as_ref().unwrap().1
                ),
                _ => "Publish process will finish in minutes.".to_owned(),
            }
        }
    }
}
