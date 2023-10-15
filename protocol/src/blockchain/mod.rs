use serde_json::json;
use types::is_valid_keys;
mod types;

const BCH_API_WRAPPER: &str = "https://fulcrum-http.pat.mn";
const BCH_API: &str = "wss://chipnet.imaginary.cash:50004";
const BCH_MIN_CONFIRMATION: i64 = 6;

pub struct Bch;

impl Bch {
    pub async fn get_tx(hash: &str) -> Result<is_valid_keys::Root, Box<dyn std::error::Error>> {
        let request = json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "blockchain.transaction.get",
            "params": [hash, true]
        });

        let response = reqwest::Client::new()
            .post(BCH_API_WRAPPER)
            .header("Content-Type", "application/json")
            .header("server", BCH_API)
            .header("accept", "*/*")
            .json(&request)
            .send()
            .await?
            .json::<is_valid_keys::Root>()
            .await?;

        Ok(response)
    }

    pub async fn is_valid_tx(
        hash: &str,
        out_hex: &str,
        out_val: f64,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let response = Bch::get_tx(hash).await?;
        for vout in response.result.vout {
            if vout.script_pub_key.hex == out_hex && vout.value == out_val {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    pub async fn is_confirmed(hash: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let response = Bch::get_tx(hash).await?;
        Ok(response.result.confirmations >= BCH_MIN_CONFIRMATION)
    }
}
