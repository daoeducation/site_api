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

mod models;

use models::Site;

use tera::Tera;

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.add_raw_template("test", include_str!("templates/checkout_sessions/show.html.tera")).expect("No static");
        tera
    };
}

#[launch]
fn rocket() -> rocket::Rocket {
  rocket::ignite()
    .mount(
      "/stripe/checkout_sessions",
      routes![
        checkout_sessions_controller::zero_to_hero,
        checkout_sessions_controller::coding_bootcamp
      ],
    )
    .attach(AdHoc::on_attach("Site config", |rocket| async {
      let site = rocket
        .figment()
        .extract::<Site>()
        .expect("Config could not be parsed")
        .validate()
        .await
        .expect("Could not validate State");

      Ok(rocket.manage(site))
    }))
}

#[cfg(test)]
use galvanic_test::test_suite;

#[cfg(test)]
test_suite! {
    name controller_specs;

    use super::models::*;
    use rocket::local::blocking::Client;

    use rocket::http::Status;
    use galvanic_assert::*;

    fixture mockstripe() -> (Vec<mockito::Mock>, guerrilla::PatchGuard) {
      setup(&mut self) {
        let mocks = vec![
          mockito::mock("GET", mockito::Matcher::Regex(".*price.*".to_string()))
            .with_body(r#"{
              "id": "price_1I7gnkDVE5TJAnJjPTABtPG3",
              "object": "price",
              "active": true,
              "billing_scheme": "per_unit",
              "created": 1610196256,
              "currency": "eur",
              "livemode": false,
              "lookup_key": null,
              "metadata": {},
              "nickname": null,
              "product": "prod_IiRoYzs4pMzzH7",
              "recurring": {
                "aggregate_usage": null,
                "interval": "month",
                "interval_count": 1,
                "usage_type": "licensed"
              },
              "tiers_mode": null,
              "transform_quantity": null,
              "type": "recurring",
              "unit_amount": 2000,
              "unit_amount_decimal": "2000"
            }"#)
            .create(),
          mockito::mock("POST", mockito::Matcher::Regex(".*checkout.*".to_string()))
            .with_body(r#"{
              "id": "cs_test_UlBpFuXAZzjRFuFKFDP5io1eC0ml0GlJUscRJCTcDPlOJzR01B5PyIRm",
              "object": "checkout.session",
              "allow_promotion_codes": null,
              "amount_subtotal": null,
              "amount_total": null,
              "billing_address_collection": null,
              "cancel_url": "https://example.com/cancel",
              "client_reference_id": null,
              "currency": null,
              "customer": null,
              "customer_email": null,
              "livemode": false,
              "locale": null,
              "metadata": {},
              "mode": "payment",
              "payment_intent": "pi_1I18y5DVE5TJAnJjdzl22po3",
              "payment_method_types": [
                "card"
              ],
              "payment_status": "unpaid",
              "setup_intent": null,
              "shipping": null,
              "shipping_address_collection": null,
              "submit_type": null,
              "subscription": null,
              "success_url": "https://example.com/success",
              "total_details": null
            }"#)
            .create(),
        ];

        let guard = guerrilla::patch1(Site::stripe, |_|{
          stripe::Client::from_url(mockito::server_url(), "sk_test_123")
        });

        (mocks, guard)
      }
    }

    fixture client() -> Client {
        setup(&mut self) {
          std::env::set_var("ROCKET_CONFIG", "Rocket.toml.example");
          let rkt = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime")
            .block_on(async { super::rocket() });

          Client::tracked(rkt).expect("valid `Rocket`")
        }
    }

    fn rematch<'a>(expr: &'a str) -> Box<dyn Matcher<'a, String> + 'a> {
      Box::new(move |actual: &String| {
        let re = regex::Regex::new(expr).unwrap();
        let builder = MatchResultBuilder::for_("rematch");
        if re.is_match(&actual) {
          builder.matched()
        } else {
          builder.failed_because(&format!("{:?} does not match {:?}", expr, actual))
        }
      })
    }

    test starts_checkout_for_zero_to_hero(mockstripe, client) {
      let response = client.val.get("/stripe/checkout_sessions/zero_to_hero").dispatch();

      assert_eq!(response.status(), Status::Ok);
      assert_that!(&response.into_string().expect("String body"), rematch("PlOJzR01B5PyIRm"))
    }

    test starts_checkout_for_coding_bootcamp(mockstripe, client) {
      let response = client.val.get("/stripe/checkout_sessions/coding_bootcamp").dispatch();

      assert_eq!(response.status(), Status::Ok);
      assert_that!(&response.into_string().expect("String body"), rematch("PlOJzR01B5PyIRm"))
    }
}
