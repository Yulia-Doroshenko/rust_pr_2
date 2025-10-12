use serde::{Deserialize, Serialize};
use std::{fs, path::Path};



use uuid;
use url;
use time;
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum RequestType {
    Success,
    Fail,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Request {
    #[serde(rename = "type")]
    pub request_type: RequestType,
    pub stream: Stream,
    pub gifts: Vec<Gift>,
    pub debug: DebugInfo,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Stream {
    pub user_id: uuid::Uuid,
    pub is_private: bool,
    pub settings: i64,
    pub shard_url: url::Url,
    pub public_tariff: PublicTariff,
    pub private_tariff: PrivateTariff,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PublicTariff {
    pub id: i64,
    pub price: i64,
    #[serde(with = "humantime_serde")]
    pub duration: std::time::Duration,      
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PrivateTariff {
    pub client_price: i64,
    #[serde(with = "humantime_serde")]
    pub duration: std::time::Duration,      
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Gift {
    pub id: i64,
    pub price: i64,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DebugInfo {
    #[serde(with = "humantime_serde")]
    pub duration: std::time::Duration,      
    #[serde(with = "time::serde::rfc3339")]
    pub at: time::OffsetDateTime,      
}

pub fn json_to_toml(json_str: &str) -> Result<String, Box<dyn std::error::Error>> {
    let req: Request = serde_json::from_str(json_str)?;
    let toml = toml::to_string_pretty(&req)?;
    Ok(toml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    #[test]
    fn parse_and_print_toml() {
        let here = Path::new(file!()).parent().unwrap();
        let json_path = here.join("request.json");
        let json = fs::read_to_string(json_path).expect("request.json");
        let toml = json_to_toml(&json).expect("to toml");
        assert!(toml.contains("type = \"success\""));
        assert!(toml.contains("shard_url = \"https://n3.example.com/sapi\""));
        assert!(toml.contains("[stream.public_tariff]"));
        let back: Request = toml::from_str(&toml).expect("from toml");
        assert_eq!(back.stream.is_private, false);
        assert_eq!(back.gifts.len(), 2);
        assert_eq!(
            back.stream.public_tariff.duration, std::time::Duration::from_secs(3600)
        );
    }
}
