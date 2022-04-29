use super::{StripePrices, DiscordSettings, WordpressSettings, BtcpaySettings, SendinblueSettings, Plans};
use serde::{Deserialize, Serialize};
use stripe::Client;
use sqlx::postgres::{PgPool, PgPoolOptions};
use crate::error::*;
use rocket::Config;

pub type Db = PgPool;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct SiteSettings {
  pub secret_key: String,
  pub checkout_domain: String,
  pub admin_key: String,
  pub payment_success_redirect: String,
  pub payment_error_redirect: String,
  pub stripe_secret_key: String,
  pub stripe_public_key: String,
  pub stripe_prices: StripePrices,
  pub stripe_events_secret: String,
  pub database_uri: String,
  pub discord: DiscordSettings,
  pub wordpress: WordpressSettings,
  pub btcpay: BtcpaySettings,
  pub sendinblue: SendinblueSettings,
  pub pricing: Plans,
}

impl SiteSettings {
  pub fn default() -> SiteSettings {
    Config::figment().extract().expect("Config could not be parsed")
  }

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

#[derive(Clone)]
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
  use sqlx::types::Decimal;
  use crate::models::{PlanCode, Plan};

  #[test]
  fn site_config_parsing() {
    let provider = Toml::string(
      r#"
        [global]
        secret_key="BEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEF"
        stripe_secret_key="sk_test_example"
        stripe_public_key = "pk_test_example"
        stripe_events_secret="supersecret"
        checkout_domain="http://example.com"
        database_uri="postgres://daoe:password@localhost/daoe_development"
        admin_key="supersecret"
        payment_success_redirect="https://dao.education/muchas-gracias"
        payment_error_redirect="https://dao.education/error-al-pagar"

        [global.pricing]
        global = { code = "global", signup = 200, degree = 500 }
        europe = { code = "europe", signup = 150, degree = 375 }
        latam = { code = "latam",  signup = 100, degree = 250 }
        guest = { code = "guest",  signup =   0, degree =   0 }

        [global.discord]
        guild_id="1000"
        bot_secret_token="SUPERSECRET"
        client_id="1002"
        student_role_id="1001"

        [global.wordpress]
        api_url="https://daocriptoacademy.com/wp-json/"
        user="user"
        pass="password"
        student_group_id=1

        [global.btcpay]
        base_url = "https://btcpay.constata.eu"
        store_id = "AAABBBCCCDDD"
        api_key = "ABCD12345"
        webhooks_secret = "SUPERSECRET"

        [global.sendinblue]
        api_key = "Sendinblueapikey"

        [global.stripe_prices]
        global_fzth_signup= "1"
        global_fzth_degree= "3"
        latam_fzth_signup= "4"
        latam_fzth_degree= "6"
        europe_fzth_signup= "7"
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
        stripe_events_secret: "supersecret".into(),
        checkout_domain: "http://example.com".into(),
        database_uri: "postgres://daoe:password@localhost/daoe_development".into(),
        payment_success_redirect: "https://dao.education/muchas-gracias".into(),
        payment_error_redirect: "https://dao.education/error-al-pagar".into(),
        admin_key: "supersecret".into(),
        pricing: Plans{
          global: Plan{
            code: PlanCode::Global,
            signup: Decimal::new(200,0),
            degree: Decimal::new(500,0),
          },
          europe: Plan{
            code: PlanCode::Europe,
            signup: Decimal::new(150,0),
            degree: Decimal::new(375,0),
          },
          latam: Plan{
            code: PlanCode::Latam,
            signup: Decimal::new(100,0),
            degree: Decimal::new(250,0),
          },
          guest: Plan{
            code: PlanCode::Guest,
            signup: Decimal::ZERO,
            degree: Decimal::ZERO,
          },
        },
        discord: DiscordSettings{
          guild_id: "1000".into(),
          bot_secret_token: "SUPERSECRET".into(),
          client_id: "1002".into(),
          student_role_id: "1001".into(),
        },
        wordpress: WordpressSettings {
          api_url: "https://daocriptoacademy.com/wp-json/".into(),
          user: "user".into(),
          pass: "password".into(),
          student_group_id: 1,
        },
        btcpay: BtcpaySettings {
          base_url: "https://btcpay.constata.eu".into(),
          store_id: "AAABBBCCCDDD".into(),
          api_key: "ABCD12345".into(),
          webhooks_secret: "SUPERSECRET".into(),
        },
        sendinblue: SendinblueSettings {
          api_key: "Sendinblueapikey".into(),
        },
        stripe_prices: StripePrices {
          global_fzth_signup: mkprice("1"),
          global_fzth_degree: mkprice("3"),
          latam_fzth_signup: mkprice("4"),
          latam_fzth_degree: mkprice("6"),
          europe_fzth_signup: mkprice("7"),
          europe_fzth_degree: mkprice("9"),
        }
      }
    );
  }
}
