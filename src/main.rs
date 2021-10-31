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

use models::{SiteSettings, Site};

use tera::Tera;

lazy_static! {
  pub static ref TEMPLATES: Tera = {
    let mut tera = Tera::default();
    tera.add_raw_template(
      "checkout_sessions/show",
      include_str!("templates/checkout_sessions/show.html.tera")
    ).expect("No static");
    tera
  };
}

pub fn server() -> rocket::Rocket<rocket::Build> {
  rocket::build()
    .mount("/payments/", routes![
      payments::handle_stripe_events,
      payments::handle_coingate_callbacks,
    ])
    .mount("/students", routes![
      students::create,
      students::show,
      students::pay_now,
    ])
    .attach(AdHoc::on_ignite("Site config", |rocket| async {
      let site = rocket
        .figment()
        .extract::<SiteSettings>()
        .expect("Config could not be parsed")
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
  use crate::{test, test_support::*};
  use super::models::*;

  test!{ full_signup_workflow(client, db) 
    let link: String = client.post("/students/",
      serde_json::json![{
        "email": "test@dao.education",
        "full_name": "Testing Testinger",
        "phone": "+23232332",
        "tax_number": "$$$$$$",
        "referral_code": "LATAMPROMO",
        "payment_method": "CoinGateBtc",
      }].to_string()
    ).await;

    let token = link[39..].to_string();
    let profile_link = link[22..].to_string();

    let profile: serde_json::Value = client.get(&profile_link).await;
    assert_eq!(1, profile.get("billing").unwrap().get("invoices").unwrap().as_array().unwrap().len());

    let invoice: Invoice = client.post(&format!("/students/pay_now?token={}", token), "").await;

    assert_that!(&invoice.url, rematch("https://pay-sandbox.coingate.com/bill/"));

    let callback = serde_json::json![{
      "id":421,
      "status":"paid",
      "paid_at":"2021-10-13T09:52:20.000Z",
      "price_amount":"130.0",
      "price_currency":"EUR",
      "created_at":"2021-10-13T09:49:53.000Z",
      "expire_at":"2021-10-18T09:49:53.000Z",
      "subscription":{
        "id":390,
        "subscription_id":"370",
        "status":"completed",
        "start_date":"2021-10-13",
        "end_date":"2021-10-13",
        "due_days_period":5,
        "created_at":"2021-10-13T09:39:22.000Z",
        "subscriber": {
          "id":370,
          "email":"test@dao.education",
          "subscriber_id":"1",
          "organisation_name":null,
          "first_name":null,
          "last_name":null,
          "address":null,
          "secondary_address":null,
          "city":null,
          "postal_code":null,
          "country":null
        },
        "details":{
          "id":365,
          "title":"DAO.Education",
          "description":"Pago mensual",
          "send_paid_notification":null,
          "payment_method":"instant",
          "price_currency":null,
          "details_id":"370",
          "receive_currency":"BTC",
          "callback_url":"https://yourcallbackurl.com",
          "items":[
            {"id":453,"item_id":"mensual","description":"pago mensual","quantity":1,"price":"30.0"},
            {"id":454,"item_id":"Inscripción","description":"Inscripción","quantity":1,"price":"100.0"}
          ]
        }
      }
    }];

    let response: String = client.post("/payments/handle_coingate_callbacks", callback.to_string()).await;

    let new_profile: serde_json::Value = client.get(&profile_link).await;
    assert!(new_profile.get("billing").unwrap().get("invoices").unwrap().as_array().unwrap().is_empty());

    dbg!(&new_profile.get("discord_verification_link"));

    //let response: serde_json::Value = client.get("/students/discord_success#token_type=Bearer&access_token=AWsigs2tu3Q9jQoiLmlDpXC7OICzdl&expires_in=604800&scope=identify+email", callback.to_string()).await;
    //  Cuando se marca como pagada una subscripcion, se hace el proceso de bienvenida.
    //    - Le manda mail de bienvenida.
    //    - A partir de ahora cuando se esté loggeado en wordpress, en la pagina.

  }

  // Crear una nueva subscripción cancela la anterior.
  // Puede crear una cuenta con stripe y recibe un IPN.
}
