use super::{Program, Site};
use serde::{Deserialize, Serialize};
use serde_json::json;
use stripe::CheckoutSessionId;
use crate::error::{Result, Error};

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutSession {
  pub program: Program,
  pub stripe_key: String,
  pub id: Option<CheckoutSessionId>,
}

impl CheckoutSession {
  pub async fn create(site: &Site, program: Program) -> Result<Self> {
    todo!("remove this");
    /*
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
    .await?;

    Ok(CheckoutSession {
      program,
      stripe_key: site.stripe_public_key.clone(),
      id: Some(stripe_session.id)
    })
    */
  }
}
