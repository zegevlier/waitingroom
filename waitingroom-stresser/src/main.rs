use futures::stream::StreamExt;
use reqwest::header::HeaderValue;

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

            let client = reqwest::Client::builder()
                .pool_max_idle_per_host(1)
                .cookie_store(true)
                .build()
                .unwrap();

            loop {
                let mut errors = 0;
                let response = loop {
                    let response = match client.get("http://127.0.0.1:8000/").send().await {
                        Ok(r) => r,
                        Err(e) => {
                            errors += 1;
                            if errors > 5 {
                                panic!("Too many errors: {}", e);
                            }
                            println!("Error: {}", e);
                            continue;
                        }
                    };
                    break response;
                };
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
        .all(|_| async { true })
        .await;
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let per_thread_count = 200;
    let parallelism_per_thread = 100;
    let thread_count = 10;

    let start_time = std::time::Instant::now();
    let mut handles = Vec::new();
    for _ in 0..thread_count {
        handles.push(tokio::spawn(run_clients(
            per_thread_count,
            parallelism_per_thread,
        )));
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    futures::future::join_all(handles).await;
    let elapsed = start_time.elapsed();

    println!(
        "Processed a total of {} users",
        per_thread_count * thread_count
    );
    println!("Elapsed time: {:?}", elapsed);
    println!(
        "Users per second: {}",
        per_thread_count * thread_count / elapsed.as_secs() as usize
    );
}
