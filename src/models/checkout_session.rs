use super::{Program, Site};
use rocket_contrib::json;
use serde::{Deserialize, Serialize};
use stripe::CheckoutSessionId;
use tokio::runtime::Runtime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutSession {
  pub program: Program,
  pub recaptcha_token: String,
  pub id: Option<CheckoutSessionId>,
}

impl CheckoutSession {
  pub async fn save(self, site: &Site) -> Option<Self> {
    Self::verify_recaptcha(&site.recaptcha_private_key, &self.recaptcha_token)?;

    let stripe_session : stripe::CheckoutSession = site.stripe().post_form("/checkout/sessions", json![{
      "success_url": format!("{}/payments/success?session_id={{CHECKOUT_SESSION_ID}}", site.checkout_domain),
      "cancel_url": format!("{}/payments/canceled", site.checkout_domain),
      "payment_method_types": ["card"],
      "mode": "payment",
      "line_items": [{
        "quantity": 1,
        "price": site.programs.price(&self.program),
      }]
    }])
    .await
    .ok()?;

    Some(CheckoutSession {
      id: Some(stripe_session.id),
      ..self
    })
  }

  pub fn verify_recaptcha(key: &str, token: &str) -> Option<()> {
    let key1 = key.to_string();
    let token1 = token.to_string();
    std::thread::spawn(move || {
      Runtime::new()
        .ok()?
        .block_on(recaptcha::verify(&key1, &token1, None))
        .ok()
    })
    .join()
    .expect("Thread panicked")
  }
}
