use rocket::{
  self,
  get,
  post,
  request::{FromRequest, Outcome, Request},
  serde::json::Json,
  State,
  http::{Status, ContentType},
  response::content::RawHtml,
  data::{self, Data, FromData, ToByteUnit},
};
use crate::error::*;
use crate::models::*;
use sha2::Sha256;
use hmac::{Hmac, Mac, NewMac};

// Create alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

pub type JsonResult<T> = Result<Json<T>>;

pub mod students;
pub mod payments;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Country {
  type Error = std::convert::Infallible;

  async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
    Outcome::Success(Country(req.headers().get_one("HTTP_CF_IPCOUNTRY").unwrap_or("XX").to_string()))
  }
}

#[rocket::async_trait]
impl<'r> FromData<'r> for btcpay::Webhook {
  type Error = Error;

  async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
    use Error::*;
    use rocket::outcome::Outcome::*;
    use rocket::data::Outcome;

    let secret = req
      .rocket()
      .state::<Site>()
      .expect("SITE not configured")
      .settings
      .btcpay
      .webhooks_secret
      .clone();

    let maybe_signature = req.headers().get_one("BTCPay-Sig").and_then(|x| hex::decode(x).ok());

    match maybe_signature {
      None => return Outcome::Forward(data),
      Some(sig) => {
        let bytes = match data.open(2048.bytes()).into_bytes().await {
          Ok(read) if read.is_complete() => read.into_inner(),
          Ok(_) => return Outcome::Failure((Status::PayloadTooLarge, Error::validation("payload", "payload too large"))),
          Err(_) => return Outcome::Failure((Status::BadRequest, Error::validation("body", "Bad request, can't read body."))),
        };

        let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
          Err(_) => return Outcome::Failure((Status::BadRequest, Error::validation("body", "Unexpected error processing hmac"))),
          Ok(a) => a
        };
        mac.update(&bytes);

        match mac.verify(&sig) {
          Err(e) => Outcome::Failure((Status::BadRequest, Error::validation("bad sig", "invalid webhook signature"))),
          _ => {
            match serde_json::from_slice(&bytes) {
              Ok(webhook) => Outcome::Success(webhook),
              _ => Outcome::Failure((Status::BadRequest, Error::validation("body", "No webhook parsed"))),
            }
          }
        }
      }
    }
  }
}
