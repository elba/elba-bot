use std::path::Path;

use failure::bail;
use git2::{build::CheckoutBuilder, Cred, PushOptions, Repository};
use log::info;

use crate::config::CONFIG;
use crate::error::{Error, Result};

pub struct Repo {
    repo: Repository,
}

impl Repo {
    pub fn clone(url: &str, checkout: &Path) -> Result<Self> {
        let repo = Repository::open(checkout).or_else(|_| {
            info!("Cloning repo {} to {:?}", url, checkout);
            let repo = Repository::clone(url, checkout);
            info!("Cloned repo {} to {:?}", url, checkout);
            repo
        })?;

        // git config
        let mut repo_cfg = repo.config()?;
        repo_cfg.set_str("user.name", &CONFIG.bot_name)?;
        repo_cfg.set_str("user.email", &CONFIG.bot_email)?;

        Ok(Repo { repo })
    }

    pub fn workdir(&self) -> Result<&Path> {
        self.repo.workdir().ok_or(Error::RepoIsBare.into())
    }

    pub fn head_hash(&self) -> String {
        hex::encode(&self.repo.head().unwrap().target().unwrap().as_bytes())
    }

    pub fn checkout(&self, refname: &str) -> Result<()> {
        // git checkout
        let commit = self.repo.revparse_single(refname)?.peel_to_commit()?;
        self.repo.set_head_detached(commit.id())?;
        self.repo
            .checkout_head(Some(CheckoutBuilder::new().force()))?;

        Ok(())
    }

    pub fn fetch_and_reset(&self) -> Result<()> {
        // git pull origin
        let mut remote = self.repo.find_remote("origin")?;
        remote.fetch(&["refs/heads/master:refs/heads/master"], None, None)?;

        // git checkout HEAD -f
        self.repo.set_head("refs/heads/master")?;
        self.repo
            .checkout_head(Some(CheckoutBuilder::new().force()))?;

        Ok(())
    }

    pub fn commit_and_push(&self, msg: &str, file: &Path) -> Result<()> {
        // git add
        let mut index = self.repo.index()?;
        index.add_path(&file.strip_prefix(self.repo.workdir().unwrap())?)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let head = self.repo.head()?;
        let parent = self
            .repo
            .find_commit(head.target().ok_or(Error::NoInitialCommit)?)?;
        let sig = self.repo.signature()?;

        // git commit -m
        self.repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&parent])?;

        // git push
        let mut remote = self.repo.find_remote("origin")?;
        let mut push_err_msg = None;
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks
            .credentials(|_, _, _| Cred::userpass_plaintext(&CONFIG.bot_email, &CONFIG.bot_pwd));
        callbacks.push_update_reference(|refname, status| {
            assert_eq!(refname, "refs/heads/master");
            push_err_msg = status.map(|s| s.to_string());
            Ok(())
        });
        remote.push(
            &["refs/heads/master"],
            Some(PushOptions::new().remote_callbacks(callbacks)),
        )?;
        if let Some(push_err_msg) = push_err_msg {
            bail!(Error::GitPush(push_err_msg));
        }

        Ok(())
    }
}
