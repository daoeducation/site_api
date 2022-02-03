use std::process::Command;
use rocket::{
  http::{Header, Status},
  local::asynchronous::Client,
};
pub use serde::{de::DeserializeOwned, Deserialize};

pub use galvanic_assert::{
  self,
  matchers::{collection::*, *},
  *,
};

use std::future::Future;
use tokio::runtime::Runtime;

pub fn run_test<F: Future<Output = Result<(), anyhow::Error>>>(future: F) {
  let result = Runtime::new()
    .expect("could not build runtime")
    .block_on(future);
  result.unwrap();
}

#[macro_export]
macro_rules! test {
  ($i:ident($client:ident, $site:ident) $($e:tt)* ) => {
    #[test]
    fn $i() {
      run_test(async move {
        crate::test_support::reset_database().await;
        let $site = daoe_api::models::SiteSettings::default().into_site().await.unwrap();
        let $client = PublicApiClient::new(crate::server()).await;
        {$($e)*};
        Ok(())
      })
    }
  }
}

pub async fn reset_database() {
  let database_uri = std::env::var("ROCKET_DATABASE_URI").unwrap_or_else(|_| {
    "postgres://daoe:password@localhost/daoe_development".to_string()
  });

  let output = Command::new("sqlx")
    .current_dir("src")
    .args(&["-D", &database_uri, "database", "reset", "-y"])
    .output()
    .unwrap();

  if !output.status.success() {
    // the -y option fails unless the script detects it's running in a terminal.
    // And for whatever reason, it detects a terminal in macos but not on linux.
    let _two = Command::new("sqlx")
      .current_dir("src")
      .args(&["-D", &database_uri, "database", "reset"])
      .output()
      .unwrap();
  }
}

#[derive(Deserialize)]
pub struct ApiError {
  pub error: String,
}

pub struct PublicApiClient {
  pub client: Client,
}

impl PublicApiClient {
  pub async fn new(server: rocket::Rocket<rocket::Build>) -> Self {
    Self {
      client: Client::tracked(server).await.unwrap(),
    }
  }

  pub async fn post<T, B>(&self, path: &str, body: B) -> T
  where
    T: DeserializeOwned,
    B: AsRef<str> + AsRef<[u8]>,
  {
    let string = self
      .client
      .post(path)
      .header(Header::new("cf-ipcountry", "AR"))
      .body(body)
      .dispatch()
      .await
      .into_string()
      .await
      .unwrap();

    serde_json::from_str(&string).unwrap_or_else(|_| panic!("Could not parse response {}", string))
  }

  pub async fn get<T: DeserializeOwned, P: std::fmt::Display>(&self, path: P) -> T {
    let response = self.raw_get(path).await;
    serde_json::from_str(&response).expect(&format!("Could not parse response {}", response))
  }

  pub async fn raw_get<P: std::fmt::Display>(&self, path: P) -> String {
    self
      .client
      .get(&path.to_string())
      .dispatch()
      .await
      .into_string()
      .await
      .unwrap()
  }

  pub async fn assert_unauthorized_get<P: std::fmt::Display>(&self, path: P) {
    let response = self.client.get(path.to_string()).dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
  }

  pub async fn assert_get_error<'a>(&'a self, path: &'a str, status: Status, msg: &'a str) {
    let response = self.client.get(path).dispatch().await;
    assert_eq!(response.status(), status);
    let err: ApiError = serde_json::from_str(&response.into_string().await.unwrap()).unwrap();
    assert_that!(&err.error, rematch(msg));
  }
}

pub fn rematch<'a>(expr: &'a str) -> Box<dyn Matcher<'a, String> + 'a> {
  Box::new(move |actual: &String| {
    let re = regex::Regex::new(expr).unwrap();
    let builder = MatchResultBuilder::for_("rematch");
    if re.is_match(actual) {
      builder.matched()
    } else {
      builder.failed_because(&format!("{:?} does not match {:?}", expr, actual))
    }
  })
}
