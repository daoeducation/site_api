use super::{Program, Site};
use rocket_contrib::json;
use serde::{Deserialize, Serialize};
use stripe::CheckoutSessionId;

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutSession {
  pub program: Program,
  pub stripe_key: String,
  pub id: Option<CheckoutSessionId>,
}

impl CheckoutSession {
  pub async fn create(site: &Site, program: Program) -> Option<Self> {
    let stripe_session : stripe::CheckoutSession = site.stripe().post_form("/checkout/sessions", json![{
      "success_url": format!("{}/payments/success?session_id={{CHECKOUT_SESSION_ID}}", site.checkout_domain),
      "cancel_url": format!("{}/payments/canceled", site.checkout_domain),
      "payment_method_types": ["card"],
      "allow_promotion_codes": true,
      "mode": "payment",
      "line_items": [{
        "quantity": 1,
        "price": site.programs.price(&program),
      }]
    }])
    .await
    .ok()?;

    Some(CheckoutSession {
      program,
      stripe_key: site.stripe_public_key.clone(),
      id: Some(stripe_session.id)
    })
  }
}
