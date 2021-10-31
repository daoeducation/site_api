use super::{StripePrices, DiscordSettings, WordpressSettings, BtcpaySettings};
use serde::{Deserialize, Serialize};
use stripe::Client;
use sqlx::postgres::{PgPool, PgPoolOptions};
use crate::error::*;

pub type Db = PgPool;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct SiteSettings {
  pub secret_key: String,
  pub checkout_domain: String,
  pub stripe_secret_key: String,
  pub stripe_public_key: String,
  pub stripe_prices: StripePrices,
  pub database_uri: String,
  pub discord: DiscordSettings,
  pub wordpress: WordpressSettings,
  pub btcpay: BtcpaySettings,
}

impl SiteSettings {
  pub async fn into_site(self) -> Result<Site> {
    let stripe = Client::new(&self.stripe_secret_key);
    self.stripe_prices.validate_all(&stripe).await?;
    let db = PgPoolOptions::new()
      .max_connections(5)
      .connect(&self.database_uri)
      .await?;

    Ok(Site{ stripe, db, settings: self })
  }
}

pub struct Site {
  pub db: Db,
  pub stripe: Client,
  pub settings: SiteSettings,
}

#[cfg(test)]
mod test {
  use super::*;
  use rocket::figment::{
    providers::{Format, Toml},
    Figment,
  };
  use std::str::FromStr;
  use stripe::PriceId;

  #[test]
  fn site_config_parsing() {
    let provider = Toml::string(
      r#"
        [global]
        secret_key="BEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEF"
        stripe_secret_key="sk_test_example"
        stripe_public_key = "pk_test_example"
        checkout_domain="http://example.com"
        database_uri="postgres://daoe:password@localhost/daoe_development"

        [global.discord]
        guild_id="1000"
        bot_secret_token="SUPERSECRET"
        client_id="1002"
        student_role_id="1001"

        [global.wordpress]
        api_url="https://user:password@daocriptoacademy.com/wp-json/"
        student_group_id=1

        [global.btcpay]
        base_url = "https://btcpay.constata.eu"
        store_id = "AAABBBCCCDDD"
        api_key = "ABCD12345"
        webhooks_secret = "SUPERSECRET"

        [global.stripe_prices]
        global_fzth_signup= "1"
        global_fzth_monthly= "2"
        global_fzth_degree= "3"
        latam_fzth_signup= "4"
        latam_fzth_monthly= "5"
        latam_fzth_degree= "6"
        europe_fzth_signup= "7"
        europe_fzth_monthly= "8"
        europe_fzth_degree= "9"
    "#,
    );

    let site: SiteSettings = Figment::new()
      .merge(provider)
      .extract_inner("global")
      .expect("Config could not be parsed");

    let mkprice = |a| { PriceId::from_str(a).unwrap() };

    assert_eq!(
      site,
      SiteSettings {
        secret_key: "BEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEF".into(),
        stripe_secret_key: "sk_test_example".into(),
        stripe_public_key: "pk_test_example".into(),
        checkout_domain: "http://example.com".into(),
        database_uri: "postgres://daoe:password@localhost/daoe_development".into(),
        discord: DiscordSettings{
          guild_id: "1000".into(),
          bot_secret_token: "SUPERSECRET".into(),
          client_id: "1002".into(),
          student_role_id: "1001".into(),
        },
        wordpress: WordpressSettings {
          api_url: "https://user:password@daocriptoacademy.com/wp-json/".into(),
          student_group_id: 1,
        },
        btcpay: BtcpaySettings {
          base_url: "https://btcpay.constata.eu".into(),
          store_id: "AAABBBCCCDDD".into(),
          api_key: "ABCD12345".into(),
          webhooks_secret: "SUPERSECRET".into(),
        },
        stripe_prices: StripePrices {
          global_fzth_signup: mkprice("1"),
          global_fzth_monthly: mkprice("2"),
          global_fzth_degree: mkprice("3"),
          latam_fzth_signup: mkprice("4"),
          latam_fzth_monthly: mkprice("5"),
          latam_fzth_degree: mkprice("6"),
          europe_fzth_signup: mkprice("7"),
          europe_fzth_monthly: mkprice("8"),
          europe_fzth_degree: mkprice("9"),
        }
      }
    );
  }
}
