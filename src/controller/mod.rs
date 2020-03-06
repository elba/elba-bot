mod command;
mod publish;

use std::fmt::Write;
use std::sync::Arc;

use log::info;
use rusqlite::Connection;
use tokio::sync::Mutex;

use self::command::Command;
use crate::config::CONFIG;
use crate::database::{self, Database};
use crate::error::Result;
use crate::github::{self, Comment, Github};
use crate::workspace::Workspace;

pub struct Controller {
    github: Arc<Github>,
    database: Mutex<Database>,
    workspace: Mutex<Workspace>,
}

impl Controller {
    pub async fn new() -> Result<Self> {
        let github = Arc::new(Github::new().await?);
        let workspace = Mutex::new(Workspace::new()?);
        let database = Database::new(Connection::open(&CONFIG.db_path)?);
        database.create_tables()?;
        let database = Mutex::new(database);
        Ok(Controller {
            github,
            database,
            workspace,
        })
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Start polling issue comments");
        let mut last_date = None;
        loop {
            // Poll comments from github issue
            let resp = self
                .github
                .query_poll(
                    github::url::issue_comments(
                        &CONFIG.index_repo_name,
                        &CONFIG.index_issue_number,
                    ),
                    &[("since", &last_date)],
                )
                .await?;

            if last_date.is_none() {
                last_date = Some(resp.date);
                continue;
            }

            let comments: Vec<Comment> = resp.val;
            for comment in comments {
                // Don't reply to early comments
                if comment.created_at < last_date.unwrap() - chrono::Duration::minutes(1) {
                    continue;
                }
                // Don't reply myself
                if comment.user.id == self.github.viewer_id() {
                    continue;
                }
                if self
                    .database
                    .lock()
                    .await
                    .query_comment(comment.id)?
                    .is_some()
                {
                    continue;
                }
                // Save comment records
                {
                    let database = self.database.lock().await;
                    database.insert_user(database::User {
                        id: comment.user.id,
                        name: comment.user.name.clone(),
                    })?;
                    database.insert_comment(database::Comment {
                        id: comment.id,
                        user_id: comment.user.id,
                        body: comment.body.clone(),
                        created_at: comment.created_at,
                    })?;
                }

                // Parse command from comment
                let command = match Command::from_str(&comment.body) {
                    Ok(Some(command)) => command,
                    Ok(None) => continue,
                    Err(_) => {
                        self.update_report(&comment, &CommandError).await?;
                        continue;
                    }
                };

                info!("Executing command: {:?}", command);

                // Execute command
                match command {
                    Command::Publish { git, refname } => {
                        let this = self.clone();
                        tokio::task::spawn(
                            async move { this.publish(git, refname, comment).await },
                        );
                    }
                }
            }

            last_date = Some(resp.date);
        }
    }

    async fn update_report<R: CommentReport>(&self, comment: &Comment, report: &R) -> Result<()> {
        let report = report.render(&comment);
        self.github.update_comment(comment.id, report).await?;
        Ok(())
    }
}

trait CommentReport {
    fn render(&self, comment: &Comment) -> String {
        let mut report = String::new();
        write!(report, "{}", &comment.body).unwrap();
        write!(report, "\n\n- - - - - - - - - - -\n\n").unwrap();
        if let Some(title) = self.render_title(&comment) {
            write!(report, "#### *{}*\n\n", title).unwrap();
        }
        if let Some(body) = self.render_body(&comment) {
            write!(report, "{}\n\n", body).unwrap();
        }
        write!(
            report,
            "@{} *{}*\n",
            comment.user.name,
            self.render_msg(&comment)
        )
        .unwrap();
        report
    }

    fn render_title(&self, comment: &Comment) -> Option<&str>;
    fn render_body(&self, comment: &Comment) -> Option<String>;
    fn render_msg(&self, comment: &Comment) -> String;
}

struct CommandError;

impl CommentReport for CommandError {
    fn render_title(&self, _: &Comment) -> Option<&str> {
        Some("Command Error")
    }

    fn render_body(&self, _: &Comment) -> Option<String> {
        None
    }

    fn render_msg(&self, _: &Comment) -> String {
        format!("elba-bot was not able to understand your command.")
    }
}

fn render_readme_package_list(database: &Database) -> Result<String> {
    let mut body = String::new();

    let mut packages: Vec<database::Package> = database.query_package(None)?;
    packages
        .sort_by(|a, b| (&a.group, &a.name, &b.version).cmp(&((&b.group, &b.name, &a.version))));
    packages.dedup_by(|a, b| (&a.group, &a.name).eq(&(&b.group, &b.name)));
    // packages.sort_by(|a, b| b.version.cmp(&a.version));

    for package in packages {
        let user_name = database.query_user(package.user_id)?.unwrap().name;
        writeln!(
            &mut body,
            "- `{}/{} {}` *{}* @[{}]({})",
            package.group,
            package.name,
            package.version,
            package
                .description
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("no description"),
            &user_name,
            github::url::user_profile(&user_name)
        )
        .unwrap();
    }

    Ok(body)
}
