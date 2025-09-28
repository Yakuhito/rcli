use chia::protocol::Bytes32;
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenHolder {
    pub address: String,
    pub amount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenHoldersResponse {
    pub status: String,
    pub tokens: Vec<TokenHolder>,
    pub page: u64,
    pub count: u64,
    pub total_count: u64,
}

pub struct SpaceScanClient {
    base_url: String,
    client: reqwest::Client,
}

impl SpaceScanClient {
    pub fn new(testnet11: bool) -> Self {
        let base_url = if testnet11 {
            "https://api-testnet11.spacescan.io/".to_string()
        } else {
            "https://api.spacescan.io/".to_string()
        };

        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    // https://docs.spacescan.io/api/cat/holders
    pub async fn get_token_holders(
        &self,
        asset_id: Bytes32,
        count: usize,
    ) -> Result<TokenHoldersResponse, reqwest::Error> {
        let url = format!(
            "{}token/holders/{}?count={}",
            self.base_url,
            hex::encode(asset_id),
            count
        );

        let response = self.client.get(&url).send().await?;

        let token_holders: TokenHoldersResponse = response.json().await?;
        Ok(token_holders)
    }
}
