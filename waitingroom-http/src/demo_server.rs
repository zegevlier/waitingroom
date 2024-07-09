use std::net::SocketAddr;

use axum::{extract::Request, response::Response, routing::get, Router};

pub(crate) async fn demo_server(listening_address: SocketAddr) {
    let app = Router::new().fallback(get(|req: Request| async move {
        log::debug!("Request to demo HTTP server");
        let mut response = Response::new(format!(
            "<div style='display:flex;flex-direction:column;align-items:center;width:100%;height:100%;background:#353552;'><h1> Congratulations! You're through the waiting room! {}</h1></div>",
            req.uri()
        ));
        response.headers_mut().insert("Content-Type", "text/html".parse().unwrap());
        response
    }));

    let listener = tokio::net::TcpListener::bind(listening_address)
        .await
        .unwrap();
    log::info!(
        "Demo HTTP server listening on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}
