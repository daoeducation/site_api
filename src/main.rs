#[macro_use]
extern crate rocket;

#[macro_use]
extern crate lazy_static;

extern crate tera;

extern crate serde_derive;
extern crate stripe;

use rocket::fairing::AdHoc;

mod controllers;
use controllers::*;
mod error;

mod models;

use models::SiteSettings;

use tera::Tera;

lazy_static! {
  pub static ref TEMPLATES: Tera = {
    let mut tera = Tera::default();
    tera.add_raw_templates([
      ("emails/welcome", include_str!("templates/emails/welcome.html.tera")),
      ("emails/payment_link", include_str!("templates/emails/payment_link.html.tera"))
    ]).expect("No static");
    tera
  };
}

pub fn server() -> rocket::Rocket<rocket::Build> {
  rocket::build()
    .mount("/payments/", routes![
      payments::handle_stripe_events,
      payments::handle_btcpay_webhooks,
      payments::from_invoice,
    ])
    .mount("/students", routes![
      students::create,
      students::show,
      students::discord_success,
    ])
    .attach(AdHoc::on_ignite("Site config", |rocket| async {
      let site = SiteSettings::default()
        .into_site()
        .await
        .expect("Could not validate site state");

      rocket.manage(site)
    }))
}

#[rocket::launch]
fn rocket() -> rocket::Rocket<rocket::Build> {
  server()
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod test {
  use crate::{models::*, test, test_support::*};
  use chrono::prelude::*;
  use chronoutil::relative_duration::RelativeDuration;

  test!{ full_signup_workflow(client, site) 
    let res = client.post::<serde_json::Value, _>("/students/",
      serde_json::json![{
        "email": "yo+testing@nubis.im",
        "full_name": "Testing Testinger",
        "phone": "+23232332",
        "tax_number": "$$$$$$",
        "tax_address": "blablabla country spain",
        "payment_method": "BtcPay",
      }].to_string()
    ).await;
    let mut state = res.get("billing").unwrap().clone();

    assert_eq!(state.get("invoices").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(state.get("unpaid_charges").unwrap().as_array().unwrap().len(), 2);
    assert_eq!(state.get("balance").unwrap().as_str().unwrap(), "-130");

    assert_that!(&state
      .get("invoices").unwrap()
      .get(0).unwrap()
      .get("url").unwrap()
      .as_str()
      .unwrap()
      .to_string(),
      rematch("btcpay.constata.eu")
    );

    client.post::<serde_json::Value, _>("/payments/from_invoice/?invoice_id=1&admin_key=adminusertoken", "").await;

    let fetch_user_billing = || async {
      let res = client.get::<serde_json::Value, _>("/students/1?admin_key=adminusertoken").await;
      res.get("billing").unwrap().clone()
    };

    state = fetch_user_billing().await;
    assert!(state.get("invoices").unwrap().as_array().unwrap().is_empty());
    assert!(state.get("unpaid_charges").unwrap().as_array().unwrap().is_empty());
    assert_eq!(state.get("balance").unwrap().as_str().unwrap(), "0");

    let student = Student::find_by_id(&site, 1).await.unwrap();
    let billing_summary = BillingSummary::new(&site, student.clone()).await.unwrap();
    billing_summary.create_monthly_charges_for(&Utc::today()).await;

    state = fetch_user_billing().await;
    assert!(state.get("invoices").unwrap().as_array().unwrap().is_empty());
    assert!(state.get("unpaid_charges").unwrap().as_array().unwrap().is_empty());
    assert_eq!(state.get("balance").unwrap().as_str().unwrap(), "0");

    for _ in 0..3 {
      billing_summary.create_monthly_charges_for(&(Utc::today() + RelativeDuration::months(1))).await;
    }

    state = fetch_user_billing().await;
    assert_eq!(state.get("invoices").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(state.get("unpaid_charges").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(state.get("balance").unwrap().as_str().unwrap(), "-30");
  }
}
