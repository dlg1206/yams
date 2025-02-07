//! File: downloader.rs
//! Description: Download files into directory for analysis
//!
//! @author Derek Garcia

use log::{debug, warn};
use reqwest::Error;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc::Sender;
use tokio::sync::Semaphore;
use tokio::time::sleep;

/// Defaults
const DEFAULT_SLEEP: Duration = Duration::from_secs(5);
const DEFAULT_BUFFER_SIZE: usize = 100;
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 5;
const DEFAULT_MAX_RETRIES: i32 = 3;

pub trait Download: Clone + Send + Sync + 'static {
    fn download(&self, url: &str, location: &str) -> Result<String, Error>;
}

async fn download_task<D: Download>(tx: Sender<String>, url: &str, location: &str, downloader: &D) {
    debug!("downloader | Processing {}", url);
    match downloader.download(url, location).await {
        Ok(file_path) => {
            tx.send(file_path).await.unwrap();
        }
        Err(_) => {
            // todo handle er
        }
    }
    debug!("downloader | Proceed {}", url);
}

#[tokio::main]
pub async fn start_download<D: Download>(
    download_queue: (Sender<String>, Receiver<String>),
    max_concurrent_requests: usize,
    max_retries_before_quit: i32,
    downloader: D,
) {
    // create tmp download dir - todo allow user to replace with their own

    // get download tx and rx
    let tx = download_queue.0;
    let rx = download_queue.1;

    let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));
    let mut cur_retires = 0;

    /*
    todo -
     This loop constantly spawns tasks. Ideally we spawn n worker tasks that listen to the
     channel, but due to limitations of tokio, we can only have one receiver at time
     */
    loop {
        match rx.try_recv() {
            Ok(url) => {
                // block until can acquire semaphore
                let permit = semaphore.clone().acquire_owned().await.unwrap();
                // create a copy of transmitter and parser and spawn async task to process the url
                let tx = tx.clone();
                let downloader = downloader.clone();

                tokio::spawn(async move {
                    // create tmp download dir - todo allow user to replace with their own
                    // creates a bunch of temp dirs :(
                    let tmp_dir_path = TempDir::new().unwrap().path().to_str().unwrap().to_string(); // todo - err handle
                    download_task(tx, &url, &tmp_dir_path, &downloader).await;
                    drop(permit);
                    // todo add to analyze queue
                });
                // reset retries if err
                if cur_retires != 0 {
                    cur_retires = 0;
                }
            }
            Err(_) => {
                // break if out of retries
                if cur_retires > max_retries_before_quit {
                    warn!("downloader | Exceeded retries: exiting. . .");
                    break;
                }
                // else sleep and try again
                warn!("downloader | No urls! Retrying. . .");
                sleep(DEFAULT_SLEEP).await;
                cur_retires += 1;
            }
        };
    }
}
