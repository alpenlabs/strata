//! Wrapper for managing a WebSocket client that supports connection recycling and client
//! restarting.

use core::fmt;

use deadpool::managed::{self, Manager, RecycleError, RecycleResult};
use jsonrpsee::{
    core::{
        async_trait,
        client::{BatchResponse, ClientT},
        params::BatchRequestBuilder,
        traits::ToRpcParams,
        ClientError, DeserializeOwned,
    },
    ws_client::{WsClient as WebsocketClient, WsClientBuilder},
};

/// Configuration for the WebSocket client.
///
/// Settings that are necessary to initialize and configure
/// the WebSocket client, ie. URL of the WebSocket server.
#[derive(Clone, Debug)]
pub struct WsClientConfig {
    /// The URL of the WebSocket server.
    pub url: String,
}

/// Manager for creating and recycling WebSocket clients.
///
/// Manages the lifecycle of WebSocket clients, including creating
/// new clients and determining whether existing clients are still valid.
#[derive(Clone, Debug)]
pub struct WsClientManager {
    /// The configuration used to initialize WebSocket clients.
    pub config: WsClientConfig,
}

/// Wrapper for the WebSocket client.
#[derive(Debug)]
pub struct WsClient {
    /// WebSocket client
    inner: WebsocketClient,
}

/// Implements the [`Manager`] trait for managing WebSocket clients.
///
/// The [`WsClientManager`] provides methods to create new clients and recycle
/// existing ones, ensuring that clients remain in a valid state.
impl Manager for WsClientManager {
    type Type = WsClient;
    type Error = jsonrpsee::core::StringError;

    /// Creates a new WebSocket client.
    ///
    /// Attempts to initialize a new WebSocket client using the provided configuration.
    /// Returns a [`WsClient`]
    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let client = WsClientBuilder::default()
            .build(self.config.url.clone())
            .await?;

        Ok(WsClient { inner: client })
    }

    /// Recycles an existing [`WsClient`]
    ///
    /// Checks whether the provided client is still valid. If the client is connected,
    /// it is retained. Otherwise, an error is returned, indicating the need to recreate
    /// the client.
    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &managed::Metrics,
    ) -> RecycleResult<Self::Error> {
        if obj.inner.is_connected() {
            Ok(())
        } else {
            Err(RecycleError::Message(
                "Connection lost, recreate client".to_string().into(),
            ))
        }
    }
}

/// Implements the [`ClientT`] trait for [`WsClient`].
///
/// This implementation allows `[WsClient`] to perform JSON-RPC operations,
/// including notifications, method calls, and batch requests.
#[async_trait]
impl ClientT for WsClient {
    /// Send a [notification request](https://www.jsonrpc.org/specification#notification).
    ///
    /// Notifications do not produce a response on the JSON-RPC server.
    async fn notification<Params>(&self, method: &str, params: Params) -> Result<(), ClientError>
    where
        Params: ToRpcParams + Send,
    {
        self.inner.notification(method, params).await
    }

    /// Send a [method call request](https://www.jsonrpc.org/specification#request_object).
    ///
    /// Returns `Ok` if the server responds successfully, otherwise a `ClientError`.
    async fn request<R, Params>(&self, method: &str, params: Params) -> Result<R, ClientError>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        self.inner.request(method, params).await
    }

    /// Sends a batch request.
    async fn batch_request<'a, R>(
        &self,
        batch: BatchRequestBuilder<'a>,
    ) -> Result<BatchResponse<'a, R>, ClientError>
    where
        R: DeserializeOwned + fmt::Debug + 'a,
    {
        self.inner.batch_request(batch).await
    }
}
