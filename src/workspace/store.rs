use std::fs::{self, File};
use std::io;
use std::path::Path;

use elba::package::{manifest::Manifest, Checksum, ChecksumFmt};
use elba::remote::resolution::DirectRes;
use failure::bail;
use log::info;
use sha2::{Digest, Sha256};

use super::Repo;
use super::*;
use crate::config::CONFIG;
use crate::error::{Error, Result};

pub struct Store {
    repo: Repo,
}

impl Store {
    pub fn clone() -> Result<Self> {
        Ok(Store {
            repo: Repo::clone(
                &github_repo_url(&CONFIG.store_repo_name),
                &CONFIG.store_checkout,
            )?,
        })
    }

    pub fn upload_package(&self, manifest: &Manifest, tarball: &Path) -> Result<DirectRes> {
        info!(
            "Uploading package `{} {}`",
            &manifest.package.name, &manifest.package.version
        );

        // Check size limit
        let size = fs::metadata(tarball)?.len();
        if size > CONFIG.store_max_size {
            bail!(Error::PackageOversize {
                size,
                limit: CONFIG.store_max_size
            });
        }

        self.repo.fetch_and_reset()?;

        // Copy tarball into local repo
        let name = &manifest.package.name;
        let tarball_dir = self
            .repo
            .workdir()?
            .join(name.normalized_group())
            .join(name.normalized_name());
        let tarball_path = tarball_dir.join(tarball_name(manifest));
        fs::create_dir_all(tarball_dir)?;
        fs::copy(tarball, &tarball_path)?;

        // Calculate the sha256 checksum
        let mut hash = Sha256::new();
        let mut file = File::open(&tarball_path)?;
        io::copy(&mut file, &mut hash)?;
        let cksum = hex::encode(hash.result());
        info!(
            "Package checksum `{} {}`: {}",
            &manifest.package.name, &manifest.package.version, &cksum
        );

        // Push update to remote
        self.repo.commit_and_push(
            &format!(
                "Update package `{} {}`",
                &manifest.package.name, &manifest.package.version
            ),
            &tarball_path,
        )?;

        let raw_url = github_raw_url(&self.repo.head_hash(), &manifest);

        // Verify github raw doanload
        info!(
            "Verifying download of package `{} {}`",
            &manifest.package.name, &manifest.package.version
        );
        let mut hash = Sha256::new();
        let mut download = reqwest::blocking::get(&raw_url)?;
        io::copy(&mut download, &mut hash)?;
        let download_cksum = hex::encode(hash.result());
        if download_cksum != cksum {
            bail!(Error::DownloadVerification {
                local_cksum: cksum,
                download_cksum
            })
        }

        info!(
            "Uploaded package `{} {}`",
            &manifest.package.name, &manifest.package.version
        );

        Ok(DirectRes::Tar {
            url: raw_url.parse()?,
            cksum: Some(Checksum {
                fmt: ChecksumFmt::Sha256,
                hash: cksum,
            }),
        })
    }
}
