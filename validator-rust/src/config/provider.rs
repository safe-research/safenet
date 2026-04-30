//! The provider type and configuration for the validator.

use alloy::{
    providers::{ProviderBuilder, RootProvider},
    rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use url::Url;

pub type Provider = RootProvider;

/// Creates a new provider with the standard configuration.
pub fn create(url: Url) -> Provider {
    let client = ClientBuilder::default()
        .layer(RetryBackoffLayer::new(10, 500, 500))
        .http(url);
    ProviderBuilder::new()
        .disable_recommended_fillers()
        .connect_client(client)
}
