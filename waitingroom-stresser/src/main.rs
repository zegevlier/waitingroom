use std::sync::Arc;

use futures::stream::StreamExt;
use reqwest::header::HeaderValue;
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};

async fn sleep_until_next_refresh(header: &HeaderValue) {
    let refresh = header.to_str().unwrap();
    let next_refresh = refresh.split(';').next().unwrap();
    let next_refresh_in: u64 = next_refresh.parse::<u64>().unwrap() * 1000;
    tokio::time::sleep(tokio::time::Duration::from_millis(next_refresh_in)).await;
}

async fn run_clients(count: usize, parallelism_per_thread: usize) {
    futures::stream::iter(0..count)
        .map(move |i| async move {
            println!("Starting client {}", i);
            let cookie_store = CookieStore::new(None);
            let cookie_store = Arc::new(CookieStoreMutex::new(cookie_store));

            let client = reqwest::Client::builder()
                .cookie_provider(Arc::clone(&cookie_store))
                .build()
                .unwrap();

            loop {
                let response = client.get("http://127.0.0.1:8000/").send().await.unwrap();
                let wr_status_header = response.headers().get("x-wr-status").unwrap();
                let wr_status = wr_status_header.to_str().unwrap();

                dbg!(wr_status);

                match wr_status {
                    s if (s == "NewTicket" || s.starts_with("TicketRefreshed(")) => {
                        sleep_until_next_refresh(response.headers().get("refresh").unwrap()).await;
                    }
                    "NewPass" => {
                        // We got the pass, so we're done!
                        break;
                    }
                    _ => {
                        panic!("Unknown status: {}", wr_status);
                    }
                }
            }
        })
        .buffer_unordered(parallelism_per_thread)
        .collect::<Vec<_>>()
        .await;
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let count = 1000;
    let parallelism_per_thread = 50;
    let thread_count = 10;

    let mut thread_handles = Vec::new();
    for _ in 0..thread_count {
        thread_handles.push(tokio::spawn(async move {
            run_clients(count / thread_count, parallelism_per_thread).await;
        }));
    }

    futures::future::join_all(thread_handles).await;
}
