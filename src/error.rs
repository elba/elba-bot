use failure::Fail;

pub type Result<T> = std::result::Result<T, failure::Error>;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Gibhub API error: {}", _0)]
    Github(String),

    #[fail(display = "No initial commit in remote index")]
    NoInitialCommit,

    #[fail(display = "Namespace `{}` has been taken by @{}", group, owner)]
    NamespaceIsTaken { group: String, owner: String },

    #[fail(display = "Package `{} {}` has been published", package, version)]
    PackageExists {
        package: String,
        version: semver::Version,
    },

    #[fail(
        display = "Package tarball is too big ({} bytes) while the maximum size is {}",
        size, limit
    )]
    PackageOversize { size: u64, limit: u64 },

    #[fail(display = "Git push failed: {}", _0)]
    GitPush(String),

    #[fail(
        display = "Package contains non-index dependency `{}`({})",
        dependency, resolution
    )]
    NonIndexDependency {
        dependency: String,
        resolution: String,
    },

    #[fail(
        display = "Package have dependency `{}` that do not exist in index",
        dependency
    )]
    DependencyNotFound { dependency: String },

    #[fail(display = "Repository is bare")]
    RepoIsBare,

    #[fail(
        display = "Tarball checksum mismatched between local {} and store {}",
        local_cksum, download_cksum
    )]
    DownloadVerification {
        local_cksum: String,
        download_cksum: String,
    },
}
