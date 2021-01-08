#[macro_use]
extern crate rocket;
extern crate serde_derive;
extern crate stripe;

use rocket::{
  fairing::{AdHoc, Fairing, Info, Kind},
  http::Header,
  Request, Response,
};

mod controllers;
use controllers::*;

mod models;

use models::Site;

#[options("/")]
pub async fn options() -> &'static str {
  ""
}

#[derive(Default)]
struct Cors();

#[rocket::async_trait]
impl Fairing for Cors {
  fn info(&self) -> Info {
    Info {
      name: "Cors",
      kind: Kind::Response,
    }
  }

  async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
    let site = req.managed_state::<Site>().unwrap();
    res.set_header(Header::new(
      "Access-Control-Allow-Origin",
      &site.checkout_domain,
    ));
    res.set_header(Header::new(
      "Access-Control-Allow-Methods",
      "POST, GET, PATCH, OPTIONS",
    ));
    res.set_header(Header::new("Access-Control-Allow-Headers", "*"));
    res.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
  }
}

#[launch]
fn rocket() -> rocket::Rocket {
  rocket::ignite()
    .mount(
      "/stripe/config",
      routes![configs_controller::index, options],
    )
    .mount(
      "/stripe/checkout_sessions",
      routes![checkout_sessions_controller::create, options],
    )
    .attach(AdHoc::on_attach("Site config", |rocket| async {
      let site: Site = rocket
        .figment()
        .extract_inner("global")
        .expect("Config could not be parsed");
      Ok(rocket.manage(site.validate().await))
    }))
    .attach(Cors())
}

#[cfg(test)]
use galvanic_test::test_suite;

#[cfg(test)]
test_suite! {
    name controller_specs;

    use super::rocket;
    use super::models::{CheckoutSession, Config, Program};
    use rocket::local::blocking::{Client, LocalResponse};
    use rocket::http::Status;
    use serde_json::{from_str};
    use serde::de::DeserializeOwned;
    use galvanic_assert::matchers::*;
    use galvanic_assert::*;
    use regex::Regex;

    fn rematch<'a>(expr: &'a str) -> Box<dyn Matcher<'a, String> + 'a> {
      Box::new(move |actual: &String| {
        let re = Regex::new(expr).unwrap();
        let builder = MatchResultBuilder::for_("rematch");
        if re.is_match(&actual) {
          builder.matched()
        } else {
          builder.failed_because(&format!("{:?} does not match {:?}", expr, actual))
        }
      })
    }

    fn j<D: DeserializeOwned>(response: LocalResponse) -> D {
      from_str(&response.into_string().expect("String body")).expect("JSON response body")
    }

    test configs_show() {
      let client = Client::tracked(super::rocket()).expect("valid `Rocket`");

      let response = client.get("/stripe/config").dispatch();
      assert_eq!(response.status(), Status::Ok);
      assert_that!(&j(response), has_structure![Config{
        stripe_key: rematch("pk_test_51I18k3DVE5TJ.*"),
        recaptcha_key: rematch("6LcxPiAaAAAA.*")
      }]);
    }

    test starts_checkout_for_zero_to_hero() {
      let _guard = guerrilla::patch2(CheckoutSession::verify_recaptcha, |_,_| Some(()) );

      let client = Client::tracked(super::rocket()).expect("valid `Rocket`");

      let response = client.post("/stripe/checkout_sessions")
        .body(r#"{"program": "ZeroToHero", "recaptcha_token": "test_token"}"#)
        .dispatch();

      assert_eq!(response.status(), Status::Ok);

      assert_that!(&j(response), has_structure![CheckoutSession{
        program: eq(Program::ZeroToHero),
        recaptcha_token: rematch("test_token")
      }]);
    }

    test starts_checkout_for_coding_bootcamp() {
      let _guard = guerrilla::patch2(CheckoutSession::verify_recaptcha, |_,_| Some(()) );

      let client = Client::tracked(super::rocket()).expect("valid `Rocket`");

      let response = client.post("/stripe/checkout_sessions")
        .body(r#"{"program": "CodingBootcamp", "recaptcha_token": "test_token"}"#)
        .dispatch();

      assert_eq!(response.status(), Status::Ok);
      assert_that!(&j(response), has_structure![CheckoutSession{
        program: is_variant!(Program::CodingBootcamp),
        recaptcha_token: rematch("test_token")
      }]);
    }

    test cannot_start_session_with_invalid_captcha() {
      let client = Client::tracked(super::rocket()).expect("valid `Rocket`");
      let response = client.post("/stripe/checkout_sessions")
        .body(r#"{"program": "CodingBootcamp", "recaptcha_token": "test_token"}"#)
        .dispatch();

      assert_eq!(response.status(), Status::NotFound);
    }
}
