use std::fmt::Display;

pub use reqwest::{self, Client as RClient, IntoUrl, Url};
use reqwest::{header, RequestBuilder};

mod pair;
pub use pair::{Liquidity, Pair, Timed, Token, Transactions};

mod response;
pub use response::{ClientError, PairResponse, Result};

/// The [Dexscreener API URL](https://docs.dexscreener.com/api/reference).
// pub const BASE_URL: &str = "https://api.dexscreener.com/latest/";
pub const BASE_URL: &str = "https://api.dexscreener.com/";

/// A [Dexscreener API](https://docs.dexscreener.com/api/reference) HTTP client.
#[derive(Clone, Debug)]
pub struct Client {
    pub client: RClient,
    pub url: Url,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Instantiate a new client with the [base URL][BASE_URL].
    pub fn new() -> Self {
        Self::with_url(BASE_URL).unwrap()
    }

    /// Instantiate a new client with the provided URL.
    pub fn with_url(url: impl IntoUrl) -> Result<Self> {
        Self::with_url_and_client(url, RClient::new())
    }

    /// Instantiate a new client with the provided URL and reqwest client.
    pub fn with_url_and_client(url: impl IntoUrl, client: RClient) -> Result<Self> {
        Ok(Self { client, url: url.into_url()? })
    }

    // MI: legacy: latest api => latest/dex/ => Result<PairResponse> is a map
    async fn get_pair(&self, path: &str) -> Result<PairResponse> {
        Ok(self._get(path)?.send().await?.error_for_status()?.json().await?)
    }

    // MI: v1 api => Result<Option<Vec<Pair>>> is an vector(sequence)
    async fn get_v1_pair(&self, path: &str) -> Result<Option<Vec<Pair>>> {
        Ok(self._get(path)?.send().await?.error_for_status()?.json().await?)
    }

    fn _get(&self, path: &str) -> Result<RequestBuilder> {
        let url = self.url.join(path)?;
        Ok(self.client.get(url).header(header::ACCEPT, "application/json"))
    }
}

/// Routes
impl Client {
    // MI: pair_addresses may be one or multiple addresses
    // if one pair_address: reuslt includes a pairs[] vec and a pair{} obj
    // if multiple pair addresses: result includes a pairs[] and null pair
    /// Performs an HTTP `GET` request to the `/dex/pairs/{chain_id}/{pair_addresses}` path.
    pub async fn pairs(
        &self,
        chain_id: impl Display,
        pair_addresses: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<PairResponse> {
        let addresses = format_addresses(pair_addresses)?;
        let path = format!("latest/dex/pairs/{chain_id}/{addresses}");
        self.get_pair(&path).await
    }

    // MI: Get the pools of a given token address
    /// Performs an HTTP `GET` request to the `token-pairs/v1/{chain_id}/{address}` path.
    pub async fn pools(&self, chain_id: impl Display, address: &str) -> Result<Option<Vec<Pair>>> {
        let path = format!("token-pairs/v1/{chain_id}/{address}");

        self.get_v1_pair(&path).await
    }

    /// Performs an HTTP `GET` request to the `/dex/tokens/{token_addresses}` path.
    pub async fn tokens(
        &self,
        token_addresses: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<PairResponse> {
        let addresses = format_addresses(token_addresses)?;
        let path = format!("latest/dex/tokens/{addresses}");
        self.get_pair(&path).await
    }

    // MI: v1_tokens is an alternative of legacy tokens
    /// Performs an HTTP `GET` request to the `/tokens/v1/{chain_id}/{token_addresses}` path.
    pub async fn v1_tokens(
        &self,
        chain_id: impl Display,
        token_addresses: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Option<Vec<Pair>>> {
        let addresses = format_addresses(token_addresses)?;
        let path = format!("tokens/v1/{chain_id}/{addresses}");

        self.get_v1_pair(&path).await
    }

    // MI: latest/dex/search?q=text
    /// Performs an HTTP `GET` request to the `/dex/search` path.
    pub async fn search(&self, query: impl AsRef<str>) -> Result<PairResponse> {
        Ok(self
            ._get("latest/dex/search")?
            .query(&[("q", query.as_ref())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

/// Formats a list of addresses into comma-separated list.
///
/// # Examples
///
/// ```
/// # use dexscreener::format_addresses;
/// assert_eq!(
///     format_addresses([
///         "0x1111111111111111111111111111111111111111",
///         "0x2222222222222222222222222222222222222222",
///     ]).unwrap(),
///     "0x1111111111111111111111111111111111111111,0x2222222222222222222222222222222222222222"
/// );
/// ```
pub fn format_addresses(
    pair_addresses: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<String> {
    let mut iter = pair_addresses.into_iter();
    let first = match iter.next() {
        Some(first) => first,
        None => return Ok(String::new()),
    };
    let cap = iter.size_hint().1.unwrap_or(5);
    let mut out = String::with_capacity(cap * 45);
    format_address(first.as_ref(), &mut out)?;
    for address in iter {
        out.push(',');
        format_address(address.as_ref(), &mut out)?;
    }
    Ok(out)
}

fn format_address(address: &str, out: &mut String) -> Result<()> {
    match address.len() {
        // Ethereum: `/(0x)?[0-9A-Fa-f]{40}/`
        40 if address.chars().all(|c| c.is_ascii_hexdigit()) => {
            out.push('0');
            out.push('x');
            out.push_str(address);
            Ok(())
        }
        42 if address.starts_with("0x")
            && address.chars().skip(2).all(|c| c.is_ascii_hexdigit()) =>
        {
            out.push_str(address);
            Ok(())
        }

        // Solana: `/[0-9A-Za-z]{44}/`
        44 if address.chars().all(|c| c.is_ascii_alphanumeric()) => {
            out.push_str(address);
            Ok(())
        }
        _ => Err(ClientError::InvalidAddress(address.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pairs() {
        let client = Client::new();
        let pair_addresses = [
            "0x7213a321F1855CF1779f42c0CD85d3D95291D34C",
            "0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae",
        ];
        let result = client.pairs("bsc", pair_addresses).await.unwrap().pairs.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_pools() {
        let client = Client::new();
        let token_address = "6p6xgHyF7AeE6TZkSmFsko444wqoP15icUSqi2jfGiPN";
        let result = client.pools("solana", token_address).await.unwrap().unwrap(); //.pairs.unwrap();
        assert_eq!(&result[0].base_token.address.clone().unwrap(), token_address);
        assert!(result.len() > 0);
    }

    #[tokio::test]
    async fn test_tokens() {
        let client = Client::new();
        let token_addresses = [
            "6p6xgHyF7AeE6TZkSmFsko444wqoP15icUSqi2jfGiPN",
            "FUAfBo2jgks6gB4Z4LfZkqSZgzNucisEHqnNebaRxM1P",
        ];
        let result = client.tokens(token_addresses).await.unwrap().pairs.unwrap();

        assert_eq!(&result[0].base_token.address.clone().unwrap(), token_addresses[0]);
        assert!(result.len() > 20);
    }

    #[tokio::test]
    async fn test_v1_tokens() {
        let client = Client::new();
        let token_addresses = [
            "6p6xgHyF7AeE6TZkSmFsko444wqoP15icUSqi2jfGiPN",
            "FUAfBo2jgks6gB4Z4LfZkqSZgzNucisEHqnNebaRxM1P",
        ];
        let result = client.v1_tokens("solana", token_addresses).await.unwrap().unwrap(); //.pairs.unwrap();
                                                                                          // println!("{:#?}", result);
        assert_eq!(&result[0].base_token.address.clone().unwrap(), token_addresses[0]);
        assert!(result.len() > 0);
    }

    #[tokio::test]
    async fn test_search() {
        let client = Client::new();
        let result = client.search("WSOL USDC").await.unwrap().pairs.unwrap();
        // println!("{:#?}", result);
        assert!(result.len() >= 1);
    }
}
