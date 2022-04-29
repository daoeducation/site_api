use rocket::{routes, fairing::AdHoc, http::Method};
use rocket_cors::{AllowedHeaders, AllowedOrigins};

use daoe_api::{
  models::SiteSettings,
  controllers::*
};

#[cfg(test)]
pub mod test_support;

pub fn server() -> rocket::Rocket<rocket::Build> {

  let allowed_origins = AllowedOrigins::some_exact(&["https://dao.education"]);

  let cors = rocket_cors::CorsOptions {
    allowed_origins,
    allowed_methods: vec![Method::Get, Method::Post, Method::Options].into_iter().map(From::from).collect(),
    allowed_headers: AllowedHeaders::some(&["Authorization", "Accept", "Content-Type"]),
    allow_credentials: true,
    ..Default::default()
  }
  .to_cors().unwrap();

  rocket::build()
    .mount("/payments/", routes![
      payments::get_pricing,
      payments::handle_stripe_events,
      payments::handle_btcpay_webhooks,
      payments::from_invoice,
    ])
    .mount("/students/", routes![
      students::discord_success,
      students::by_wordpress_id,
      students::create,
      students::create_guest,
      students::show,
      students::index,
    ])
    .mount("/", rocket_cors::catch_all_options_routes())
    .attach(cors.clone())
    .manage(cors)
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
mod test {
  use daoe_api::{models::*};
  use crate::{test, test_support::*};
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
    assert_eq!(state.get("unpaid_charges").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(state.get("balance").unwrap().as_str().unwrap(), "-100");

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

    let student = site.student().find(&1).await.unwrap();
    BillingSummary::new(student.clone()).await.unwrap();

    state = fetch_user_billing().await;
    assert!(state.get("invoices").unwrap().as_array().unwrap().is_empty());
    assert!(state.get("unpaid_charges").unwrap().as_array().unwrap().is_empty());
    assert_eq!(state.get("balance").unwrap().as_str().unwrap(), "0");
  }
}
