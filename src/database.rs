use chrono::{DateTime, FixedOffset};
use rusqlite::{params, Connection};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_rusqlite::{from_rows, to_params_named};

use crate::error::Result;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(conn: Connection) -> Database {
        Database { conn }
    }

    pub fn create_tables(&self) -> Result<()> {
        self.conn.execute(
            "
                CREATE TABLE IF NOT EXISTS users (
                    id INTERGER PRIMARY KEY,
                    name VARCHAR NOT NULL
                );
            ",
            params![],
        )?;
        self.conn.execute(
            "
                CREATE TABLE IF NOT EXISTS packages (
                    group_name VARCHAR NOT NULL,
                    name VARCHAR NOT NULL,
                    version VARCHAR NOT NULL,
                    description VARCHAR,
                    user_id INTERGER NOT NULL,

                    UNIQUE(group_name, name, version)
                    FOREIGN KEY (user_id)
                        REFERENCES users (id)
                );
            ",
            params![],
        )?;
        self.conn.execute(
            "
                CREATE TABLE IF NOT EXISTS comments (
                    id INTERGER PRIMARY KEY,
                    user_id INTERGER NOT NULL,
                    body VARCHAR NOT NULL,
                    created_at VARCHAR NOT NULL,
                    
                    FOREIGN KEY (user_id)
                        REFERENCES users (id)
                );
            ",
            params![],
        )?;
        Ok(())
    }

    pub fn query_user(&self, user_id: i64) -> Result<Option<User>> {
        let mut stat = self.conn.prepare(
            "
                SELECT * FROM users WHERE id = ?1;
            ",
        )?;
        let mut rows = from_rows::<User>(stat.query(params![user_id])?);
        Ok(rows.next().transpose()?)
    }

    pub fn insert_user(&self, user: User) -> Result<()> {
        self.conn.execute_named(
            "
                INSERT OR REPLACE INTO users (id, name)
                VALUES (:id, :name)
            ",
            &to_params_named(user)?.to_slice(),
        )?;
        Ok(())
    }

    pub fn query_package(&self, group: Option<&str>) -> Result<Vec<Package>> {
        let selection = if let Some(group) = group {
            format!("WHERE group_name = \"{}\"", group)
        } else {
            "".to_owned()
        };

        let mut stat = self.conn.prepare(&format!(
            "
                SELECT * FROM packages {};
            ",
            selection
        ))?;

        let rows = from_rows::<Package>(stat.query(params![])?);
        let rows: Result<Vec<_>> = rows
            .into_iter()
            .map(|row| row.map_err(Into::into))
            .collect();
        Ok(rows?)
    }

    pub fn insert_package(&self, package: Package) -> Result<()> {
        self.conn.execute_named(
            "
                INSERT INTO packages (group_name, name, version, description, user_id)
                VALUES (:group_name, :name, :version, :description, :user_id)
            ",
            &to_params_named(package)?.to_slice(),
        )?;
        Ok(())
    }

    pub fn query_comment(&self, comment_id: i64) -> Result<Option<Comment>> {
        let mut stat = self.conn.prepare(
            "
                SELECT * FROM comments WHERE id = ?1;
            ",
        )?;
        let mut rows = from_rows::<Comment>(stat.query(params![comment_id])?);
        Ok(rows.next().transpose()?)
    }

    pub fn insert_comment(&self, comment: Comment) -> Result<()> {
        self.conn.execute_named(
            "
                INSERT INTO comments (id, user_id, body, created_at)
                VALUES (:id, :user_id, :body, :created_at)
            ",
            &to_params_named(comment)?.to_slice(),
        )?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Comment {
    pub id: i64,
    pub user_id: i64,
    pub body: String,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    #[serde(rename = "group_name")]
    pub group: String,
    pub name: String,
    pub version: Version,
    pub description: Option<String>,
    pub user_id: i64,
}
