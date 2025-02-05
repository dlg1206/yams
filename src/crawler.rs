//! File: crawler.rs
//! Description: Generic crawler logic for downloading HTML files
//!
//! @author Derek Garcia

use reqwest::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::Semaphore;

/// Defaults
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36";
const DEFAULT_SLEEP: Duration = Duration::from_secs(5);
const DEFAULT_BUFFER_SIZE: usize = 100;
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 5;
const DEFAULT_MAX_RETRIES: i32 = 3;

/// Download the html content at the provided url
///
/// # Parameters
/// - `url`: The url to download the html from
///
/// # Returns
/// - `Ok(String)`: The html content
/// - `Err(Error)`: The error if the download fails.
async fn download_html(url: &str) -> Result<String, Error> {
    let response = reqwest::Client::new()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;
    let html = response.text().await?;
    Ok(html)
}

/// Process the provided url and add newly parse and download urls into their respective channels
///
/// # Parameters
/// - `tx`: The transmitter to the parse url channel
/// - `url`: The url to process
/// - `parser`: Custom parser to parse the html at the url
async fn process_url<P: Parser>(tx: Sender<String>, url: String, parser: P) {
    let html = download_html(&url).await.unwrap(); // todo err handle
    let (parse_urls, download_urls) = parser.parse(&url, &html);
    // send the new parse urls to parse channel
    for new_url in parse_urls {
        tx.send(new_url.to_owned()).await.unwrap();
    }
    // todo download
}

///
/// Parser trait that must be implemented by users
///
pub trait Parser: Clone + Send + Sync + 'static {
    /// Custom parser method that parses html to generate the next urls to process
    ///
    /// # Parameters
    /// - `source_url`: The source url of the provided html
    /// - `html`: The html content of the url
    ///
    /// # Returns
    /// - `Vec<String>`: The list of urls that can be further parsed
    /// - `Vec<String>`: The list of urls that can be downloaded
    fn parse(&self, source_url: &str, html: &str) -> (Vec<String>, Vec<String>);
}

///
/// Generic Crawler Struct
///
#[derive(Debug)]
pub struct Crawler {
    max_concurrent_requests: usize,
    max_retries_before_quit: i32,
}

///
/// Default Crawler Implementation
///
impl Default for Crawler {
    /// Create a generic crawler using the default values
    ///
    /// # Returns
    /// - Default Crawler
    fn default() -> Self {
        Self {
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            max_retries_before_quit: DEFAULT_MAX_RETRIES,
        }
    }
}

///
/// Crawler
///
impl Crawler {
    /// Create new crawler with none default variables
    ///
    /// # Parameters
    /// - `max_concurrent_requests`: The maximum requests allowed at one time
    /// - `max_retries_before_quit`: The number of times to attempt and fail to pull from the parser url channel before exiting
    pub fn new(max_concurrent_requests: usize, max_retries_before_quit: i32) -> Self {
        Self {
            max_concurrent_requests,
            max_retries_before_quit,
        }
    }

    /// Main crawl method. Spawn tasks to process urls
    ///
    /// # Parameters
    /// - `root_url`: The root url to start crawling at
    /// - `parser`: Custom parser to parse the html at the url
    #[tokio::main]
    pub async fn crawl<P: Parser>(&self, root_url: String, parser: P) {
        // create parse url queue
        let (tx, mut rx) = channel::<String>(DEFAULT_BUFFER_SIZE);
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_requests));

        // add seed url
        tx.send(root_url).await.unwrap();

        // todo - This loop constantly spawns tasks. Ideally we spawn n worker tasks that listen to the channel
        while let Some(url) = rx.recv().await {
            // block until can acquire semaphore
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            // create a copy of transmitter and parser and spawn task to process the url
            let tx = tx.clone();
            let parser = parser.clone();
            tokio::spawn(async move {
                process_url(tx, url, parser).await;
                drop(permit);
            });
        }
    }
}
