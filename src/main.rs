#![feature(async_closure)]
#![feature(try_trait)]
#![feature(specialization)]
#![feature(try_blocks)]

mod config;
mod controller;
mod database;
mod error;
mod github;
mod workspace;

use std::sync::Arc;
use std::time::Duration;

use log::{error, info};

use crate::controller::Controller;
use crate::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    loop {
        info!("Controller started");
        let res = tokio::spawn(async {
            let res: Result<_> = try {
                let controller = Arc::new(Controller::new().await?);
                controller.run().await?;
            };
            res
        })
        .await?;

        if let Err(err) = res {
            error!("Controller failure: {}", err);
            tokio::time::delay_for(Duration::from_secs(5)).await;
        }
    }
}
