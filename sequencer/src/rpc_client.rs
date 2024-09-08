use jsonrpsee::{core::client::async_client::Client, ws_client::WsClientBuilder};

pub async fn sync_client(url: &str) -> Client {
    WsClientBuilder::default()
        .build(url)
        .await
        .expect("Failed to connect to the RPC server")
}
