use axum::{extract::Request, routing::get, Router};

pub(crate) async fn demo_server(listening_address: SocketAddr) {
    let app = Router::new().fallback(get(|req: Request| async move {
        log::debug!("Request to demo HTTP server");
        format!(
            "Congratulations! You're through the waiting room! {}",
            req.uri()
        )
    }));

    let listener = tokio::net::TcpListener::bind(std::net::SocketAddr::from(listening_address))
        .await
        .unwrap();
    log::info!(
        "Demo HTTP server listening on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}
