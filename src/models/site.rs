use super::{Program, Programs};
use serde::{Deserialize, Serialize};
use stripe::{Client, Price};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Site {
  pub secret_key: String,
  pub checkout_domain: String,
  pub stripe_secret_key: String,
  pub stripe_public_key: String,
  pub programs: Programs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SiteError {
  InvalidPrice(Program),
  NoSiteInState,
}

impl Site {
  pub fn stripe(&self) -> Client {
    Client::new(&self.stripe_secret_key)
  }

  pub async fn validate(self) -> Result<Self, SiteError> {
    let client = self.stripe();
    for program in [Program::ZeroToHero, Program::CodingBootcamp].iter() {
      Price::retrieve(&client, &self.programs.price(&program), &[])
        .await
        .map_err(move |_| SiteError::InvalidPrice(*program))?;
    }
    Ok(self)
  }
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

        [global.programs]
        zero_to_hero = "price_one"
        coding_bootcamp = "price_two"
    "#,
    );

    let site: Site = Figment::new()
      .merge(provider)
      .extract_inner("global")
      .expect("Config could not be parsed");

    assert_eq!(
      site,
      Site {
        secret_key: "BEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEFBEEF".into(),
        stripe_secret_key: "sk_test_example".into(),
        stripe_public_key: "pk_test_example".into(),
        checkout_domain: "http://example.com".into(),
        programs: Programs {
          zero_to_hero: PriceId::from_str("price_one").unwrap(),
          coding_bootcamp: PriceId::from_str("price_two").unwrap()
        }
      }
    );
  }
}
