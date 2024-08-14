//! Defines the `Execute` trait for the operator which encapsulates all bridge duties that an
//! operator must execute.

use alpen_express_rpc_api::AlpenBridgeApiClient;
use async_trait::async_trait;

/// A meta trait that encapsulates all the traits that a bridge operator must implement and wires
/// them together.
#[async_trait]
pub trait Execute: AlpenBridgeApiClient {
    // TODO: Define the functionalities of a challenger.
}
