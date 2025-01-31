//! File: crawler.rs
//! Description: Generic crawler logic for pulling HTML files
//!
//! @author Derek Garcia

use reqwest::Error;
use std::collections::HashMap;
/// Represents a Crawler
///
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use tokio::time::sleep;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36";
const DEFAULT_SLEEP: Duration = Duration::from_secs(5);

pub struct Crawler {
    url_queue: (Sender<String>, Receiver<String>),
    max_concurrent_requests: i32,
    max_retries_before_quit: i32,
}

pub trait Parser {
    fn parse(&self, source_url: &str, html: &str) -> (Vec<String>, Vec<String>);
}

impl Crawler {
    pub fn new(
        url_queue: (Sender<String>, Receiver<String>),
        max_concurrent_requests: i32,
        max_retries_before_quit: i32,
    ) -> Self {
        Self {
            url_queue,
            max_concurrent_requests,
            max_retries_before_quit,
        }
    }

    async fn download_html(&self, url: &str) -> Result<String, Error> {
        let response = reqwest::Client::new()
            .get(url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;
        let html = response.text().await?;
        Ok(html)
    }

    #[tokio::main]
    pub async fn crawl<P: Parser>(&self, root_url: String, parser: &P) {
        let mut cur_retries: i32 = 0;
        self.url_queue.0.send(root_url).unwrap(); // 0 means sender - todo more graceful unwrap
        loop {
            println!("Attempting to get url");
            match self.url_queue.1.try_recv() {
                Ok(url) => {
                    println!("Got url!");
                    cur_retries = 0;
                    match self.download_html(&url).await {
                        Ok(html) => {
                            let (new_parse_urls, new_download_urls) = parser.parse(&url, &html);
                            new_parse_urls
                                .iter() // Iterate over `new_parse_urls`
                                .for_each(|x| self.url_queue.0.send(x.to_owned()).unwrap());
                            // For each element, send it to the queue
                            // todo add download queue
                        }
                        Err(e) => {
                            eprintln!("Error downloading HTML: {}", e);
                        }
                    };
                }
                Err(_) => {
                    if cur_retries >= self.max_retries_before_quit {
                        println!("Exceed retries");
                        break;
                    }
                    println!("No url found, waiting...");
                    sleep(DEFAULT_SLEEP).await;
                    cur_retries += 1;
                }
            }
        }
        println!("Finished!");
    }
}
