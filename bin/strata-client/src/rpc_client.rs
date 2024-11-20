use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

pub fn sync_client(url: &str) -> HttpClient {
    HttpClientBuilder::default()
        .build(url)
        .expect("Failed to connect to the RPC server")
}
