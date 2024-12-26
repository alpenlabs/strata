//! Wrapper for managing a WebSocket client that allows recycling or restarting
//! the client without restarting the entire program.

use core::fmt;

use deadpool::managed::{self, Manager, RecycleError, RecycleResult};
use jsonrpsee::{
    core::{
        async_trait,
        client::{BatchResponse, ClientT},
        params::BatchRequestBuilder,
        traits::ToRpcParams,
        BoxError, ClientError, DeserializeOwned,
    },
    ws_client::{WsClient as WebsocketClient, WsClientBuilder},
};

/// Configuration for the WebSocket client.
///
/// settings that are necessary to initialize and configure
/// the WebSocket client, ie. URL of the WebSocket server.
#[derive(Clone, Debug)]
pub struct WsClientConfig {
    /// The URL of the WebSocket server.
    pub url: String,
}

/// Manager for creating and recycling WebSocket clients.
///
/// This struct manages the lifecycle of WebSocket clients, including creating
/// new clients and determining whether existing clients are still valid.
#[derive(Clone, Debug)]
pub struct WsClientManager {
    /// The configuration used to initialize WebSocket clients.
    pub config: WsClientConfig,
}

/// Represents the state of a WebSocket client.
///
/// - `Working`: The client is connected and operational.
/// - `NotWorking`: The client is not connected or has encountered an error.
#[derive(Debug)]
pub enum WsClientState {
    /// The WebSocket client is connected and operational.
    Working(WebsocketClient),
    /// The WebSocket client is not connected or is in a failed state.
    NotWorking,
}

/// Wrapper for the WebSocket client state.
///
/// This struct encapsulates the `WsClientState`, enabling unified management of
/// both connected and failed client states.
#[derive(Debug)]
pub struct WsClient(WsClientState);

/// Implements the `Manager` trait for managing WebSocket clients.
///
/// The `WsClientManager` provides methods to create new clients and recycle
/// existing ones, ensuring that clients remain in a valid state.
impl Manager for WsClientManager {
    type Type = WsClient;
    type Error = jsonrpsee::core::StringError;

    /// Creates a new WebSocket client.
    ///
    /// Attempts to initialize a new WebSocket client using the provided configuration.
    /// Returns a `WsClient` wrapped in a `Working` or `NotWorking` state depending on
    /// the success of the operation.
    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let client = WsClientBuilder::default()
            .build(self.config.url.clone())
            .await;
        let bl = match client {
            Ok(cl) => WsClientState::Working(cl),
            Err(_) => WsClientState::NotWorking,
        };
        Ok(WsClient(bl))
    }

    /// Recycles an existing WebSocket client.
    ///
    /// Checks whether the provided client is still valid. If the client is connected,
    /// it is retained. Otherwise, an error is returned, indicating the need to recreate
    /// the client.
    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &managed::Metrics,
    ) -> RecycleResult<Self::Error> {
        match &obj.0 {
            WsClientState::Working(cl) => {
                if cl.is_connected() {
                    Ok(())
                } else {
                    Err(RecycleError::Message(
                        "Connection lost, recreate client".to_string().into(),
                    ))
                }
            }
            WsClientState::NotWorking => Err(RecycleError::Message(
                "Connection still not found, recreate client"
                    .to_string()
                    .into(),
            )),
        }
    }
}

/// Implements the `ClientT` trait for `WsClient`.
///
/// This implementation allows `WsClient` to perform JSON-RPC operations,
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
        match &self.0 {
            WsClientState::Working(inner) => inner.notification(method, params).await,
            WsClientState::NotWorking => Err(ClientError::Transport(BoxError::from(
                "Client is Not Working".to_string(),
            ))),
        }
    }

    /// Send a [method call request](https://www.jsonrpc.org/specification#request_object).
    ///
    /// Returns `Ok` if the server responds successfully, otherwise a `ClientError`.
    async fn request<R, Params>(&self, method: &str, params: Params) -> Result<R, ClientError>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        match &self.0 {
            WsClientState::Working(inner) => inner.request(method, params).await,
            WsClientState::NotWorking => Err(ClientError::Transport(BoxError::from(
                "Client is Not Working".to_string(),
            ))),
        }
    }

    /// Sends a batch request.
    async fn batch_request<'a, R>(
        &self,
        batch: BatchRequestBuilder<'a>,
    ) -> Result<BatchResponse<'a, R>, ClientError>
    where
        R: DeserializeOwned + fmt::Debug + 'a,
    {
        match &self.0 {
            WsClientState::Working(inner) => inner.batch_request(batch).await,
            WsClientState::NotWorking => Err(ClientError::Transport(BoxError::from(
                "Client is Not Working".to_string(),
            ))),
        }
    }
}
