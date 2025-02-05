//! File: crawler.rs
//! Description: Generic crawler logic for pulling HTML files
//!
//! @author Derek Garcia

use reqwest::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::Semaphore;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36";
const DEFAULT_SLEEP: Duration = Duration::from_secs(5);
const DEFAULT_BUFFER_SIZE: usize = 100;
const DEFAULT_MAX_CONCURRENT_REQUESTS: i32 = 5;
const DEFAULT_MAX_RETRIES: i32 = 3;

#[derive(Debug)]
pub struct Crawler {
    max_concurrent_requests: i32,
    max_retries_before_quit: i32,
}

impl Default for Crawler {
    fn default() -> Self {
        Self {
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            max_retries_before_quit: DEFAULT_MAX_RETRIES,
        }
    }
}

pub trait Parser: Clone + Send + Sync + 'static {
    fn parse(&self, source_url: &str, html: &str) -> (Vec<String>, Vec<String>);
}

async fn download_html(url: &str) -> Result<String, Error> {
    let response = reqwest::Client::new()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;
    let html = response.text().await?;
    Ok(html)
}

async fn producer<P: Parser>(tx: Sender<String>, url: String, parser: P) {
    let html = download_html(&url).await.unwrap(); // todo err handle
    let (parse_urls, download_urls) = parser.parse(&url, &html);

    for new_url in parse_urls {
        tx.send(new_url.to_owned()).await.unwrap();
    }
    // todo download
}

impl Crawler {
    // todo add builder
    pub fn new(max_concurrent_requests: i32, max_retries_before_quit: i32) -> Self {
        Self {
            max_concurrent_requests,
            max_retries_before_quit,
        }
    }

    #[tokio::main]
    pub async fn crawl<P: Parser>(&self, root_url: String, parser: P) {
        let (tx, mut rx) = channel::<String>(DEFAULT_BUFFER_SIZE);
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_requests as usize));
        tx.send(root_url).await.unwrap();

        /*
        todo - This loop constantly spawns tasks. Ideally we spawn n worker tasks that listen
        to the channel
        */
        while let Some(url) = rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let tx = tx.clone();
            let parser = parser.clone();
            tokio::spawn(async move {
                producer(tx, url, parser).await;
                drop(permit);
            });
        }
    }
}
