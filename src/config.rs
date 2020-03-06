use std::path::PathBuf;

use failure::ResultExt as _;
use lazy_static::lazy_static;
use serde_derive::Deserialize;

use crate::error::Result;

lazy_static! {
    pub static ref CONFIG: Config = Config::from_env().unwrap();
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub db_path: PathBuf,
    pub bot_name: String,
    pub bot_email: String,
    pub bot_pwd: String,
    pub access_token: String,
    pub store_repo_name: String,
    pub index_repo_name: String,
    pub index_issue_number: String,
    pub index_checkout: PathBuf,
    pub store_checkout: PathBuf,
    pub store_max_size: u64,
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(envy::from_env().context("while reading from environment")?)
    }
}
