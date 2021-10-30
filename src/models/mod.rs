use chrono::{DateTime, Date, Utc, Duration};
use serde::{Deserialize, Serialize, ser::{Serializer, SerializeStruct}};
use sqlx::{
  types::Decimal,
  postgres::{PgArguments, Postgres},
  Database,
};
use validator::Validate;
use crate::error::{Result, Error};
pub use stripe::{PriceId, Price, Customer, CustomerId};
pub use rocket::{
  http::{uri::Path, Status},
  request::{FromRequest, Outcome, Request},
};
pub mod site;
pub use site::*;
use rocket::http::uri::Reference;

pub type UtcDateTime = DateTime<Utc>;
pub type UtcDate = Date<Utc>;

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Validate)]
pub struct PublicStudentForm {
  #[validate(email)]
  pub email: String,
  pub full_name: String,
  pub phone: Option<String>,
  pub tax_number: Option<String>,
  pub tax_address: Option<String>,
  pub referral_code: Option<String>,
  pub payment_method: PaymentMethod,
}

impl PublicStudentForm {
  pub async fn save(&self, site: &Site, country: Country) -> Result<Student> {
    self.validate()?;

    let tx = site.db.begin().await?;
    let student = sqlx::query_as!(Student,
      r#"INSERT INTO students (
        email,
        full_name,
        phone,
        tax_number,
        tax_address,
        referral_code,
        country,
        payment_method,
        created_at
      ) VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, now() )
      RETURNING
        id,
        email,
        full_name,
        country,
        created_at,
        phone,
        tax_number,
        tax_address,
        referral_code,
        current_subscription_id,
        wordpress_user,
        wordpress_initial_password,
        discord_user_id,
        discord_handle,
        discord_verification,
        stripe_customer_id,
        "payment_method" as "payment_method!: PaymentMethod"
      "#,
      self.email,
      self.full_name,
      self.phone,
      self.tax_number,
      self.tax_address,
      self.referral_code,
      country.0,
      self.payment_method as PaymentMethod,
    ).fetch_one(&site.db).await?;

    student.subscribe(&site, &country.plan()).await?;

    tx.commit().await?;
    Ok(student)
  }
}

#[derive(Debug)]
pub struct Student {
  pub id: i32,
  pub email: String,
  pub full_name: String,
  pub country: String,
  pub created_at: UtcDateTime,
  pub current_subscription_id: Option<i32>,
  pub phone: Option<String>,
  pub tax_number: Option<String>,
  pub tax_address: Option<String>,
  pub referral_code: Option<String>,
  pub wordpress_user: Option<String>,
  pub wordpress_initial_password: Option<String>,
  pub discord_verification: Option<String>,
  pub discord_user_id: Option<String>,
  pub discord_handle: Option<String>,
  pub stripe_customer_id: Option<String>,
  pub payment_method: PaymentMethod,
}

#[derive(Debug, Serialize)]
pub struct StudentState<'a> {
  discord_verification_link: Option<String>,
  billing: BillingSummary<'a>,
}

impl StudentState<'_> {
  pub fn new(site: &Site, student: Student) -> Self {
    let discord_verification_link = student.discord_verification.map(|token|{
      format!( "https://discord.com/api/oauth2/authorize?response_type=token&client_id={}&state={}&scope=identify%20email%20guilds.join&redirect_uri={}/students/discord_success",
      site.discord_client_id,
      &token,
      site.checkout_domain,
    )});
    let billing = BillingSummary::new(site, student);

    Self{ discord_verification_link, billing }
  }
}

#[derive(Default, Clone)]
pub struct StudentQuery {
  id: Option<i32>,
  stripe_customer_id: Option<String>,
}

#[derive(FromForm)]
struct DiscordToken {
  state: String,
  access_token: String,
}

#[derive(Deserialize)]
struct DiscordProfile {
  id: String,
  username: String,
  discriminator: i32,
  avatar: String,
  verified: String,
  email: String,
}

// Hub has methods for find, all, create, update, delete.
// It always knows the site and database.
// find(query: &QueryStudent)
// all(query: &QueryStudent)
// create(form: CreateStudent)
// update(form: UpdateStudent{ field: Option<Option> })
// struct Student {} : AsStudentId
// struct StudentId {} : AsStudentId
// trait AsStudentId {}
//
// struct<'a> StudentHub(&'a Site);
// -- Make nice signatures for the query type.
// -- A macro that generates a macro?
// -- The hub is actually anything implementing "As Db"

impl Student {
  pub fn query<'a>(
    query: &StudentQuery,
  ) -> sqlx::query::Map<
    'a,
    Postgres,
    impl FnMut(<Postgres as Database>::Row) -> std::result::Result<Student, sqlx::error::Error>
      + Send,
    PgArguments,
  > {
    sqlx::query_as!(
      Student,
      r#"SELECT
        id,
        email,
        full_name,
        country,
        created_at,
        phone,
        tax_number,
        tax_address,
        referral_code,
        current_subscription_id,
        wordpress_user,
        discord_user_id,
        discord_verification,
        stripe_customer_id,
        "payment_method" as "payment_method: PaymentMethod"
        FROM students
        WHERE
          ($1::int4 IS NULL OR id = $1::int4)
          AND
          ($2::varchar IS NULL OR stripe_customer_id = $2::varchar)
        "#,
      query.id,
      query.stripe_customer_id,
    )
  }

  pub async fn find(site: &Site, q: &StudentQuery) -> sqlx::Result<Student> {
    Student::query(q).fetch_one(&site.db).await
  }

  pub async fn find_optional(site: &Site, q: &StudentQuery) -> sqlx::Result<Option<Student>> {
    Student::query(q).fetch_optional(site).await
  }

  pub async fn find_by_id(site: &Site, id: i32) -> sqlx::Result<Student> {
    Student::find(site, &StudentQuery{ id: Some(id), ..Default::default()} ).await
  }

  pub async fn make_profile_link(&self, site: &Site, domain: &str, hours: i64) -> Result<String> {
    Ok(SessionToken::create(&site, self.id, 72).await?.as_profile_link(domain))
  }

  pub async fn get_or_create_stripe_customer_id(&self, client: &stripe::Client, site: &Site) -> Result<CustomerId> {
    use std::collections::HashMap;
    use stripe::CreateCustomer;

    if let Some(ref id) = self.stripe_customer_id {
      return Ok(id.parse::<CustomerId>()?);
    }

    let mut metadata = HashMap::new();
    metadata.insert("student_id".to_string(), self.id.to_string());
    let customer_id = Customer::create(client, CreateCustomer{
      email: Some(&self.email),
      metadata: Some(metadata), 
      ..Default::default()
    }).await?.id;
    sqlx::query!("UPDATE students SET stripe_customer_id = $1 WHERE id = $2",
      Some(customer_id.to_string()),
      self.id
    ).execute(&site.db).await?;
    Ok(customer_id)
  }

  pub async fn subscription(&self, site: &Site) -> sqlx::Result<Subscription> {
    sqlx::query_as!(Subscription, 
      r#"SELECT 
      id,
      created_at,
      student_id,
      active,
      price,
      paid,
      paid_at, 
      invoicing_day,
      "plan_code" as "plan_code!: PlanCode",
      stripe_subscription_id
      FROM subscriptions WHERE student_id = $1 AND active"#,
      self.id
    ).fetch_one(&site.db).await
  }

  async fn subscribe(&self, site: &Site, plan: &Plan) -> Result<(Subscription, MonthlyCharge)> {
    let subscription = sqlx::query_as!(Subscription,
      r#"INSERT INTO subscriptions (student_id, active, plan_code, price)
        VALUES ($1, true, $2, $3)
        RETURNING
          id,
          created_at,
          student_id,
          active,
          price,
          paid,
          paid_at,
          invoicing_day,
          "plan_code" as "plan_code!: PlanCode",
          stripe_subscription_id
      "#,
      self.id,
      plan.code.clone() as PlanCode,
      plan.signup
    ).fetch_one(&site.db).await?;

    let monthly_charge = sqlx::query_as!(MonthlyCharge,
      "INSERT INTO monthly_charges (student_id, created_at, subscription_id, price)
        VALUES ($1, now(), $2, $3)
        RETURNING *
      ",
      self.id,
      subscription.id,
      plan.monthly
    ).fetch_one(&site.db).await?;

    Ok((subscription, monthly_charge))
  }

  pub async fn setup_wordpress(&self, site: &Site) -> Result<()> {
    let password = gen_passphrase();

    struct WordpressUser {
      id: i32,
    }

    let user: WordpressUser = ureq::post(&format!("{}/wp/v2/users/", site.wordpress.api_url))
      .send_json(serde_json::json!({
        "username": self.full_name,
        "password": password,
        "email": self.email,
      }))?
      .into_json()?;

    sqlx::query("UPDATE students SET wordpress_user = $1, wordpress_initial_password = $2")
      .execute(&site.db).await?;

    ureq::post(&format!("{}/ldlms/v2/users/{}/groups", site.wordpress.api_url, user.id))
      .send_json(serde_json::json!({"group_id":[site.wordpress.student_group_id]}))?;
  }

  pub async fn process_discord_response(site: &Site, discord_data: &str) -> Result<String> {
    let discord: DiscordToken = rocket::form::Form::parse(discord_data)?;

    let student = Student::find(&site, StudentQuery{ discord_verification: Some(discord.state), ..Default::default()}).await;
    let profile: DiscordProfile = ureq::get("https://discord.com/api/v9/users/@me")
      .set("Authorization", format!("Bearer {}", discord.access_token))
      .call()?
      .into_json()?;

    let add_url = format!("https://discord.com/api/v9/guilds/{}/members/{}", site.discord.guild_id, profile.id);

    let _ignored_because_it_may_be_member = ureq::request("PUT", &add_url)
      .set("Authorization", format!("Bot {}", site.discord.bot_secret_token))
      .send_json(serde_json::json![{"access_token": discord.access_token}])?;

    let _ignored_because_it_may_have_role = ureq::request("PUT", &format!("{}/roles/{}", &add_url, site.discord.student_role_id))
      .set("Authorization", format!("Bot {}", site.discord.bot_secret_token))
      .send()?;

    sqlx::query!(
      "UPDATE students SET discord_handle = $2, discord_user_id = $3 WHERE id = $1",
      student.id,
      format!("{}#{}", profile.username, profile.discriminator),
      profile.id
    ).execute(&site.db).await?;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
  id: i32,
  created_at: UtcDateTime,
  invoicing_day: i32,
  student_id: i32,
  active: bool,
  price: Decimal,
  paid: bool,
  plan_code: PlanCode,
  paid_at: Option<UtcDateTime>,
  stripe_subscription_id: Option<String>,
}

impl Subscription {
  pub fn plan(&self) -> Plan {
    Plan::by_code(self.plan_code)
  }

  pub async fn on_paid(&self, site: &Site) -> Result<()> {
    sqlx::query!(
      "UPDATE students SET discord_verification = $2 WHERE id = $1",
      self.student_id,
      gen_passphrase()
    ).execute(&site.db).await?;

    let student = Student::find_by_id(self.student_id);

    student.setup_wordpress(site, site);
  }

  pub fn next_invoicing_date(&self) -> UtcDate {
    use chrono::prelude::*;
    use chronoutil::relative_duration::RelativeDuration;
    let today = Utc::today();
    let this_months = Utc.ymd(today.year(), today.month(), self.invoicing_day as u32);
    if today >= this_months {
      this_months + RelativeDuration::months(1)
    }else{
      this_months
    }
  }
}

#[derive(Debug, Clone)]
pub struct MonthlyCharge {
  id: i32,
  created_at: UtcDateTime,
  student_id: i32,
  subscription_id: i32,
  price: Decimal,
  paid: bool,
  paid_at: Option<UtcDateTime>,
}

pub struct Session {
  pub token: SessionToken,
  pub student: Student,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Session {
  type Error = ();

  async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
    async fn build(req: &Request<'_>) -> Option<Session> {
      let site = req.rocket().state::<Db>()?;
      let token_str = req.query_value::<&str>("token").and_then(|r| r.ok())?;
      SessionToken::consume(site, &token_str).await.ok()
    }

    match build(req).await {
      Some(session) => Outcome::Success(session),
      None => Outcome::Failure((Status::Unauthorized, ())),
    }
  }
}

pub struct SessionToken {
  id: i32,
  student_id: i32,
  value: String,
  expires_on: UtcDateTime,
}

impl SessionToken {
  pub async fn create(site: &Site, student_id: i32, hours: i64) -> sqlx::Result<Self> {
    sqlx::query_as!(SessionToken,
      "INSERT INTO session_tokens (student_id, value, expires_on) VALUES ($1, $2, $3) RETURNING *",
      student_id,
      gen_passphrase(),
      Utc::now() + Duration::hours(hours),
    ).fetch_one(&site.db).await
  }

  pub fn as_profile_link(&self, domain: &str) -> String {
    format!("{}/students/?token={}", domain, self.value)
  }

  pub async fn consume(site: &Site, value: &str) -> sqlx::Result<Session> {
    let token = sqlx::query_as!(SessionToken,
      "SELECT * FROM session_tokens WHERE value = $1 AND expires_on > now()", value
    ).fetch_one(&site.db).await?;

    let student = token.student(site).await?;

    Ok(Session{token, student})
  }

  pub async fn student(&self, site: &Site) -> sqlx::Result<Student> {
    Student::find_by_id(site, self.student_id).await
  }
}

#[derive(Debug, Clone)]
pub struct Degree {
  id: i32,
  subscription_id: i32,
  student_id: i32,
  created_at: UtcDateTime,
  description: String,
  poap_link: Option<String>,
  constata_certificate_id: Option<String>,
  price: Decimal,
  paid: bool,
  paid_at: Option<UtcDateTime>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Payment {
  id: i32,
  student_id: i32,
  created_at: UtcDateTime,
  amount: Decimal,
  fees: Decimal,
  payment_method: PaymentMethod,
  clearing_data: String,
}

#[derive(Debug, Default)]
pub struct PaymentQuery {
  id: Option<i32>,
  student_id: Option<i32>,
}

impl Payment {
  pub fn query<'a>(
    query: PaymentQuery,
  ) -> sqlx::query::Map<
    'a,
    Postgres,
    impl FnMut(<Postgres as Database>::Row) -> std::result::Result<Payment, sqlx::error::Error>
      + Send,
    PgArguments,
  > {
    sqlx::query_as!(Payment,
      r#"SELECT
        id,
        student_id,
        created_at,
        amount,
        fees,
        "payment_method" as "payment_method!: PaymentMethod",
        clearing_data
        FROM payments
        WHERE
          ($1::int4 IS NULL OR id = $1::int4)
          AND
          ($2::int4 IS NULL OR student_id = $2::int4)
        "#,
      query.id,
      query.student_id,
    )
  }

  pub async fn create(
    site: &Site,
    student_id: i32,
    amount: Decimal,
    fees: Decimal,
    payment_method: PaymentMethod,
    clearing_data: &str,
    invoice_id: Option<i32>,
  ) -> sqlx::Result<Payment> {
    let payment = sqlx::query_as!(Payment,
      r#"INSERT INTO payments (
        student_id,
        created_at,
        amount,
        fees,
        payment_method,
        clearing_data
      ) VALUES ($1, now(), $2, $3, $4, $5)
      RETURNING 
        id,
        student_id,
        created_at,
        amount,
        fees,
        "payment_method" as "payment_method!: PaymentMethod",
        clearing_data
      "#,
      &student_id,
      amount,
      fees,
      payment_method as PaymentMethod,
      clearing_data,
    ).fetch_one(&site.db).await?;

    if let Some(id) = invoice_id {
      sqlx::query!(
        "UPDATE invoices SET paid = true, payment_id = $2 WHERE id = $1", 
        id,
        payment.id,
      ).execute(&site.db).await?;
    }

    BillingSummary::new(site, Student::find_by_id(student_id).await?).await?.apply_payments().await?;

    Ok(payment)
  }

  pub async fn from_stripe_event(e: &stripe::Event, site: &Site) -> Result<Option<Payment>> {
    use stripe::{Event, EventType, EventObject};

    if let (EventType::InvoicePaymentSucceeded, EventObject::Invoice(i)) = (&e.event_type, &e.data.object) {
      if !i.paid.unwrap_or(false) {
        return Ok(None);
      }

      let customer_id = i.customer.as_ref().map(|c| c.id().to_string() ).ok_or(Error::validation("customer","missing"))?;
      let maybe_student = Student::find_optional(site, &StudentQuery{
        stripe_customer_id: Some(customer_id),
        ..Default::default()
      }).await?;

      if let Some(student) = maybe_student {
        let amount = Decimal::new(i.amount_paid.ok_or(Error::validation("amount_paid", "missing"))?, 2);
        let maybe_invoice = Invoice::query(&InvoiceQuery{
          amount: Some(amount),
          student_id: Some(student.id),
          payment_method: Some(PaymentMethod::Stripe),
        }).fetch_optional(&site.db).await?;

        Ok(Some(Payment::create(
          site,
          student.id,
          amount,
          Decimal::new(i.tax.unwrap_or(0), 2),
          PaymentMethod::Stripe,
          &serde_json::to_string(&i)?,
          maybe_invoice
        ).await?))
      } else {
        Ok(None)
      }
    } else {
      Ok(None)
    }
  }

  pub async fn from_btcpay_webhook(webhook: &btcpay::Webhook, site: &Site) -> Result<Option<Payment>> {
    let maybe_invoice = Invoice::query(&InvoiceQuery{
      external_id: Some(webhook.id),
      payment_method: Some(PaymentMethod::BtcPay),
    }).fetch_optional(&site.db).await?;

    if let Some(invoice) = maybe_invoice {
      Ok(Some(Payment::create(
        site,
        invoice.student_id,
        invoice.amount,
        Decimal::ZERO,
        PaymentMethod::BtcPay,
        "".to_string(),
        None,
      ).await?))
    } else {
      Ok(None)
    }
  }
}

/// An invoice is a request for payment from a customer. 
/// Invoices are built for a single amount that includes all pending charges at the moment.
/// Customers have a balance with us, so it's possible that a given incoming payment is not
/// applied to payment for some specific items.
/// So all invoices should be sent for "charges pending up to DD/MM/YYYY, including..."
/// instead of "Item A, Item B, Item C".
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Invoice {
  pub id: i32,
  pub student_id: i32,
  pub created_at: UtcDateTime,
  pub payment_method: PaymentMethod,
  pub external_id: String,
  pub amount: Decimal,
  pub description: String,
  pub url: String,
  pub paid: bool,
  pub payment_id: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct InvoiceQuery {
  id: Option<i32>,
  student_id: Option<i32>,
  external_id: Option<String>,
  amount: Option<Decimal>,
  payment_method: Option<PaymentMethod>,
}

impl Invoice {
  pub fn query<'a>(
    query: InvoiceQuery,
  ) -> sqlx::query::Map<
    'a,
    Postgres,
    impl FnMut(<Postgres as Database>::Row) -> std::result::Result<Invoice, sqlx::error::Error>
      + Send,
    PgArguments,
  > {
    sqlx::query_as!(Invoice,
      r#"SELECT
        id,
        student_id,
        created_at,
        amount,
        "payment_method" as "payment_method!: PaymentMethod",
        description,
        external_id,
        url,
        paid,
        payment_id
        FROM invoices
        WHERE
          ($1::int4 IS NULL OR id = $1::int4)
          AND
          ($2::int4 IS NULL OR student_id = $2::int4)
          AND
          ($3::varchar IS NULL OR external_id = $3::varchar)
          AND
          ($4::decimal IS NULL OR amount = $4::decimal)
          AND
          ($5::payment_method IS NULL OR payment_method = $5::payment_method)
          AND
          NOT paid AND NOT expired
        "#,
      query.id,
      query.student_id,
      query.external_id,
      query.amount,
      query.payment_method as Option<PaymentMethod>,
    )
  }

  pub async fn create(
    site: &Site,
    student_id: i32,
    amount: Decimal,
    payment_method: PaymentMethod,
    description: &str,
    url: &str,
    external_id: &str,
  ) -> sqlx::Result<Invoice> {
    sqlx::query_as!(Invoice,
      r#"INSERT INTO invoices (
        student_id,
        created_at,
        amount,
        payment_method,
        description,
        url,
        external_id
      ) VALUES ($1, now(), $2, $3, $4, $5, $6)
      RETURNING 
        id,
        student_id,
        created_at,
        amount,
        "payment_method" as "payment_method!: PaymentMethod",
        description,
        external_id,
        url,
        paid,
        payment_id
      "#,
      &student_id,
      amount,
      payment_method as PaymentMethod,
      description,
      url,
      external_id,
    ).fetch_one(&site.db).await
  }
}

#[rocket::async_trait]
pub trait BillingCharge: Send + Sync + std::fmt::Debug {
  fn description(&self) -> String;
  fn created_at(&self) -> UtcDateTime;
  fn amount(&self) -> Decimal;
  fn paid_at(&self) -> Option<UtcDateTime>;
  fn set_paid(&mut self, site: &Site) -> sqlx::Result<()>;
  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId;
}

impl Serialize for dyn BillingCharge {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 3 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("BillingHistoryItem", 4)?;
        state.serialize_field("created_at", &self.created_at())?;
        state.serialize_field("description", &self.description())?;
        state.serialize_field("amount", &self.amount())?;
        state.serialize_field("paid_at", &self.paid_at())?;
        state.end()
    }
}

#[rocket::async_trait]
impl BillingCharge for MonthlyCharge {
  fn description(&self) -> String {
    "Cargo mensual".to_string()
  }

  fn created_at(&self) -> UtcDateTime {
    self.created_at.clone()
  }

  fn amount(&self) -> Decimal {
    self.price.clone()
  }

  fn paid_at(&self) -> Option<UtcDateTime> {
    self.paid_at.clone()
  }

  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId {
    prices.monthly
  }

  async fn set_paid(&mut self, site: &Site) -> sqlx::Result<()> {
    self.paid_at = Utc.now();
    self.paid = true;
    sqlx::query!(
      "UPDATE monthly_charges SET paid = true, paid_at = $2 WHERE id = $1",
      self.paid_at,
      self.id
    ).execute(&site.db).await?;
  }
}

#[rocket::async_trait]
impl BillingCharge for Degree {
  fn description(&self) -> String {
    "Titulación".to_string()
  }

  fn created_at(&self) -> UtcDateTime {
    self.created_at.clone()
  }

  fn amount(&self) -> Decimal {
    self.price.clone()
  }

  fn paid_at(&self) -> Option<UtcDateTime> {
    self.paid_at.clone()
  }

  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId {
    prices.degree
  }

  async fn set_paid(&mut self, site: &Site) -> sqlx::Result<()> {
    self.paid_at = Utc.now();
    self.paid = true;
    sqlx::query!(
      "UPDATE degrees SET paid = true, paid_at = $2 WHERE id = $1",
      self.paid_at,
      self.id
    ).execute(&site.db).await?;
  }
}

#[rocket::async_trait]
impl BillingCharge for Subscription {
  fn description(&self) -> String {
    "Titulación".to_string()
  }

  fn created_at(&self) -> UtcDateTime {
    self.created_at.clone()
  }

  fn amount(&self) -> Decimal {
    self.price.clone()
  }

  fn paid_at(&self) -> Option<UtcDateTime> {
    self.paid_at.clone()
  }

  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId {
    prices.signup
  }

  async fn set_paid(&mut self, site: &Site) -> sqlx::Result<()> {
    self.paid_at = Utc.now();
    self.paid = true;
    sqlx::query!(
      "UPDATE subscriptions SET paid = true, paid_at = $2 WHERE id = $1",
      self.paid_at,
      self.id
    ).execute(&site.db).await?;
  }
}

pub trait BillingHistoryItem: Send + Sync + std::fmt::Debug {
  fn date(&self) -> UtcDateTime;
  fn description(&self) -> String;
  fn amount(&self) -> Decimal;
}

impl Serialize for dyn BillingHistoryItem {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 3 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("BillingHistoryItem", 3)?;
        state.serialize_field("date", &self.date())?;
        state.serialize_field("description", &self.description())?;
        state.serialize_field("amount", &self.amount())?;
        state.end()
    }
}

impl<T: BillingCharge> BillingHistoryItem for T {
  fn date(&self) -> UtcDateTime {
    self.created_at()
  }
  fn description(&self) -> String {
    self.description()
  }
  fn amount(&self) -> Decimal {
    self.amount() * Decimal::NEGATIVE_ONE
  }
}

impl BillingHistoryItem for Payment {
  fn date(&self) -> UtcDateTime {
    self.created_at.clone()
  }

  fn description(&self) -> String {
    format!("Payment #{} via #{:?}", self.id, self.payment_method)
  }

  fn amount(&self) -> Decimal {
    self.amount
  }
}

#[derive(Debug, Serialize)]
pub struct BillingSummary<'a> {
  #[serde(skip_serializing)]
  pub student: Student,
  #[serde(skip_serializing)]
  pub site: &'a Site,

  pub subscription: Subscription,
  pub history: Vec<Box<dyn BillingHistoryItem>>,
  pub unpaid_charges: Vec<Box<dyn BillingCharge>>,
  pub invoices: Vec<Invoice>,
  pub total_charges_not_invoiced_yet: Option<Decimal>,
  pub balance: Decimal,
}

impl<'a> BillingSummary<'a> {
  pub async fn new(site: &'a Site, student: Student) -> Result<BillingSummary<'a>> {
    let mut unpaid_charges: Vec<Box<dyn BillingCharge>> = vec![];
    let mut history: Vec<Box<dyn BillingHistoryItem>> = vec![];

    let subscription = student.subscription(&site).await?;

    history.push(Box::new(subscription.clone()));

    if !subscription.paid {
      unpaid_charges.push(Box::new(subscription.clone()));
    }

    let degrees = sqlx::query_as!(Degree,
      "SELECT * FROM degrees WHERE student_id = $1",
      student.id
    ).fetch_all(&site.db).await?;

    for degree in degrees.into_iter() {
      if !degree.paid {
        unpaid_charges.push(Box::new(degree.clone()));
      }
      history.push(Box::new(degree));
    }

    let monthly_charges = sqlx::query_as!(MonthlyCharge,
      "SELECT * FROM monthly_charges WHERE student_id = $1",
      student.id
    ).fetch_all(&site.db).await?;

    for charge in monthly_charges.into_iter() {
      if !charge.paid {
        unpaid_charges.push(Box::new(charge.clone()));
      }
      history.push(Box::new(charge));
    }

    let payments = Payment::query(PaymentQuery{ id: None, student_id: Some(student.id)}).fetch_all(&site.db).await?;

    for payment in payments.into_iter() {
      history.push(Box::new(payment))
    }

    let balance: Decimal = history.iter().map(|i| i.amount() ).sum();
    let invoices = Invoice::query(
      InvoiceQuery{ id: None, student_id: Some(student.id), ..Default::default()}
    ).fetch_all(&site.db).await?;
    let invoiced: Decimal = invoices.iter().map(|i| i.amount ).sum();
    let total_charges_not_invoiced_yet = if balance.is_negative() { Some(balance + invoiced) } else { None };

    Ok(BillingSummary {
      subscription,
      student,
      site,
      history,
      unpaid_charges,
      invoices,
      total_charges_not_invoiced_yet,
      balance
    })
  }

  pub async fn set_payment_method(&mut self, payment_method: PaymentMethod) -> Result<()> {
    sqlx::query!(
      "UPDATE students SET payment_method = $2 WHERE id = $1",
      self.student.id,
      payment_method as PaymentMethod,
    ).execute(&self.site.db).await?;
    self.student = Student::find_by_id(self.site, self.student.id).await?;
    Ok(())
  }

  pub async fn invoice_all_not_invoiced_yet(&self) -> Result<Option<Invoice>> {
    if self.subscription.plan_code == PlanCode::Guest {
      return Ok(None)
    }

    let amount = match self.total_charges_not_invoiced_yet {
      Some(a) => a,
      None => return Ok(None),
    };

    let maybe_url_and_external_id = match self.student.payment_method {
      PaymentMethod::Stripe => self.request_on_stripe().await?,
      PaymentMethod::BtcPay => self.request_on_btcpay().await?,
      _ => None
    };

    match maybe_url_and_external_id {
      Some((url, external_id)) => Ok(Some(Invoice::create(
        self.site,
        self.student.id,
        amount,
        self.student.payment_method,
        "Cargos pendientes",
        &url,
        &external_id).await?
      )),
      _ => Ok(None),
    }
  }

  pub async fn invoice_everything(&mut self) -> Result<Option<Invoice>> {
    sqlx::query!("UPDATE invoices SET expired = true WHERE student_id = $1", self.student.id)
      .execute(&self.site.db).await?;
    self.total_charges_not_invoiced_yet = if self.balance.is_negative() { Some(self.balance) } else { None };
    self.invoice_all_not_invoiced_yet().await
  }

  pub async fn apply_payments(self) -> Result<Self> {
    let mut available = self.balance.clone();
    if available.is_positive() {
      return Ok(())
    }

    for charge in self.unpaid_charges.iter_mut() {
      if charge.amount() < available {
        charge.set_paid(&self.site.db).await?;
        available -= charge.amount();
      } else {
        break;
      }
    }

    self.balance = available;
    self.unpaid_charges = self.unpaid_charges.iter().filter(|c| c.paid_at.is_none() ).collect();
    Ok(self)
  }

  async fn request_on_stripe(&self) -> Result<Option<(String, String)>> {
    use serde_json::json;
    pub use stripe::{CheckoutSession, Subscription, ListSubscriptions, SubscriptionStatusFilter};

    let client = &self.site.stripe;
    let prices = self.site.settings.stripe_prices.by_plan_code(self.subscription.plan_code);
    let customer_id = self.student.get_or_create_stripe_customer_id(&client, &self.site).await?;

    let subscribed = Subscription::list(client, ListSubscriptions{
      customer: Some(customer_id.clone()),
      status: Some(SubscriptionStatusFilter::Active),
      ..ListSubscriptions::new()
    }).await?.total_count.unwrap_or(0) > 0;

    let line_items: Vec<&PriceId> = self.unpaid_charges.iter()
      .map(|i| i.stripe_price(&prices) ).collect();

    let stripe_session : CheckoutSession = client.post_form("/checkout/sessions", json![{
      "success_url": format!("{}/payments/success", self.site.settings.checkout_domain),
      "cancel_url": format!("{}/payments/canceled", self.site.settings.checkout_domain),
      "customer": customer_id,
      "payment_method_types": ["card"],
      "mode": "subscription",
      "line_items": line_items.into_iter().map(|i| json![{"quantity": 1, "price": i.clone()}]).collect::<Vec<serde_json::Value>>(),
    }])
    .await?;

    Ok(Some((stripe_session.url, stripe_session.id.to_string())))
  }

  async fn request_on_btcpay(&self) -> Result<Option<(String, String)>> {
    let total = self.total_charges_not_invoiced_yet
      .ok_or(Error::validation("amount","Cannot request empty amount"))?;

    let invoice: btcpay::Invoice = ureq::post(&format!("{}/invoices", self.site.settings.btcpay.url))
      .set("Authorization", &format!("token {}", self.site.settings.btcpay.api_key))
      .send_json(serde_json::to_value(btcpay::InvoiceForm{ amount: total, currency: Currency::Eur })?)?
      .into_json()?;

    Ok(Some((invoice.checkout_link, invoice.id)))
  }
}

#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[sqlx(type_name = "payment_method", rename_all = "lowercase")]
pub enum PaymentMethod {
  Stripe,
  BtcPay,
}

pub enum Currency {
  Eur,
  Btc,
}

impl PaymentMethod {
  fn quoted_currency(self) -> Currency {
    match self {
      PaymentMethod::Stripe => Currency::Eur,
      PaymentMethod::BtcPay => Currency::Btc,
    }
  }
}

pub struct Country(pub String);

impl Country {
  pub fn plan(&self) -> Plan {
    let latam = vec![
      "AR", "BH", "BO", "BR", "BZ", "CL", "CO", "CR", "EC", "FK", "GF", "GY",
      "GT", "HN", "MX", "NI", "PA", "PY", "PE", "SR", "SV", "UY", "VE",
    ];

    let europe = vec![
      "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR",
      "HU", "IE", "IT", "LV", "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK",
      "SI", "ES", "SE", "GB", 
    ];

    let code = if latam.contains(&self.0.as_str()) {
      PlanCode::Latam
    } else if europe.contains(&self.0.as_str()) {
      PlanCode::Europe
    } else {
      PlanCode::Global
    };

    Plan::by_code(code)
  }
}

#[derive(sqlx::Type, PartialEq, Copy, Clone, Debug, Deserialize, Serialize)]
#[sqlx(type_name = "PlanCode")]
pub enum PlanCode {
  Global,
  Europe,
  Latam,
  Guest,
}

pub struct Plan {
  code: PlanCode,
  signup: Decimal,
  monthly: Decimal,
  degree: Decimal,
}

impl Plan {
  fn by_code(code: PlanCode) -> Plan {
    match code {
      PlanCode::Global => Plan{
        code,
        signup: Decimal::new(200,0),
        monthly: Decimal::new(60,0),
        degree: Decimal::new(500,0),
      },
      PlanCode::Europe => Plan{
        code,
        signup: Decimal::new(150,0),
        monthly: Decimal::new(45,0),
        degree: Decimal::new(375,0),
      },
      PlanCode::Latam => Plan{
        code,
        signup: Decimal::new(100,0),
        monthly: Decimal::new(30,0),
        degree: Decimal::new(250,0),
      },
      PlanCode::Guest => Plan{
        code,
        signup: Decimal::ZERO,
        monthly: Decimal::ZERO,
        degree: Decimal::ZERO,
      }
    }
  }
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct DiscordSettings {
  guild_id: String,
  bot_secret_token: String,
  client_id: String,
  student_role_id: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct WordpressSettings {
  api_url: String,
  student_group_id: i32,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct BtcpaySettings {
  base_url: String,
  store_id: String,
  api_key: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct StripePrices {
  global_fzth_signup: PriceId,
  global_fzth_monthly: PriceId,
  global_fzth_degree: PriceId,
  latam_fzth_signup: PriceId,
  latam_fzth_monthly: PriceId,
  latam_fzth_degree: PriceId,
  europe_fzth_signup: PriceId,
  europe_fzth_monthly: PriceId,
  europe_fzth_degree: PriceId,
}

#[derive(Debug, PartialEq)]
struct StripePlanPrices<'a> {
  signup: &'a PriceId,
  monthly: &'a PriceId,
  degree: &'a PriceId,
}

impl StripePrices {
  pub async fn validate_all(&self, client: &stripe::Client) -> Result<()> {
    let prices = vec![
      &self.global_fzth_signup,
      &self.global_fzth_monthly,
      &self.global_fzth_degree,
      &self.latam_fzth_signup,
      &self.latam_fzth_monthly,
      &self.latam_fzth_degree,
      &self.europe_fzth_signup,
      &self.europe_fzth_monthly,
      &self.europe_fzth_degree,
    ];
    for price in prices {
      Price::retrieve(client, price, &[]).await
        .map_err(move |_| Error::InvalidStripePrice)?;
    }
    Ok(())
  }

  fn by_plan_code<'a>(&'a self, code: PlanCode) -> StripePlanPrices<'a> {
    match code {
      PlanCode::Europe => StripePlanPrices{
        signup: &self.europe_fzth_signup,
        monthly: &self.europe_fzth_monthly,
        degree: &self.europe_fzth_degree,
      },
      PlanCode::Latam => StripePlanPrices{
        signup: &self.latam_fzth_signup,
        monthly: &self.latam_fzth_monthly,
        degree: &self.latam_fzth_degree,
      },
      _ => StripePlanPrices {
        signup: &self.global_fzth_signup,
        monthly: &self.global_fzth_monthly,
        degree: &self.global_fzth_degree,
      }
    }
  }
}

pub fn gen_passphrase() -> String {
  use chbs::{config::BasicConfig, prelude::*};
  let mut config = BasicConfig::default();
  config.separator = "+".into();
  config.capitalize_first = false.into();
  config.to_scheme().generate()
}

pub mod btcpay {
  use super::*;

  #[derive(Debug, PartialEq, Clone, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum WebhookType {
    InvoiceCreated,
    InvoiceReceivedPayment,
    InvoicePaidInFull,
    InvoiceExpired,
    InvoiceSettled,
    InvoiceInvalid,
  }

  #[derive(Debug, PartialEq, Clone, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub struct Webhook {
    pub deliveryId: String,
    pub webhookId: String,
    pub originalDeliveryId: String,
    pub isRedelivery: bool,
    #[serde(rename = "type")]
    pub kind: WebhookType,
    pub timestamp: UtcDateTime,
    pub storeId: String,
    pub invoiceId: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub struct Invoice {
    pub id: String,
    pub checkout_link: String,
  }

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "snake_case")]
  pub struct InvoiceForm {
    pub amount: Decimal,
    pub currency: Currency,
  }
}
