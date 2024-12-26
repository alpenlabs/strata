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

#[derive(Clone, Debug)]
pub struct WsClientConfig {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct WsClientManager {
    pub config: WsClientConfig,
}

#[derive(Debug)]
pub enum WsClientState {
    Working(WebsocketClient),
    NotWorking,
}

#[derive(Debug)]
pub struct WsClient(WsClientState);

impl Manager for WsClientManager {
    type Type = WsClient;

    type Error = jsonrpsee::core::StringError;

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

    /// Send a [batch request](https://www.jsonrpc.org/specification#batch).
    ///
    /// The responses to the batch are returned in the same order as the requests were inserted.
    ///
    /// Returns `Ok` if all requests in the batch were answered, otherwise `Err(ClientError)`.
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
                "Client is NotWorking".to_string(),
            ))),
        }
    }
}
