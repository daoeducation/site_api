use crate::TEMPLATES;
use chrono::{DateTime, Date, Utc};
use chronoutil::relative_duration::RelativeDuration;
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

#[derive(Debug, Clone)]
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

#[derive(Serialize)]
pub struct StudentState<'a> {
  pub discord_verification_link: Option<String>,
  pub billing: BillingSummary<'a>,
}

impl<'a> StudentState<'a> {
  pub async fn new(site: &'a Site, student: Student) -> Result<StudentState<'a>> {
    Ok(Self{
      discord_verification_link: student.discord_verification_link(site),
      billing: BillingSummary::new(site, student).await?,
    })
  }
}

#[derive(Default, Clone)]
pub struct StudentQuery {
  id: Option<i32>,
  stripe_customer_id: Option<String>,
  discord_verification: Option<String>,
}

#[derive(FromForm)]
pub struct DiscordToken {
  pub state: String,
  pub access_token: String,
}

#[derive(Deserialize)]
pub struct DiscordProfile {
  pub id: String,
  pub username: String,
  pub discriminator: i32,
  pub avatar: String,
  pub verified: String,
  pub email: String,
}

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
        wordpress_initial_password,
        discord_user_id,
        discord_handle,
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
    Student::query(q).fetch_optional(&site.db).await
  }

  pub async fn find_by_id(site: &Site, id: i32) -> sqlx::Result<Student> {
    Student::find(site, &StudentQuery{ id: Some(id), ..Default::default()} ).await
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
    let subscription = Subscription::create(self.id, plan, site).await?;
    let monthly_charge = subscription.create_monthly_charge(site, &Utc::today()).await?;
    Ok((subscription, monthly_charge))
  }

  pub async fn setup_discord_verification(&mut self, site: &Site) -> Result<()> {
    let pass = gen_passphrase();
    sqlx::query!(
      "UPDATE students SET discord_verification = $2 WHERE id = $1",
      self.id,
      pass,
    ).execute(&site.db).await?;
    self.discord_verification = Some(pass);
    Ok(())
  }

  pub async fn setup_wordpress(&mut self, site: &Site) -> Result<()> {
    if self.wordpress_user.is_some() {
      return Ok(())
    }

    let password = gen_passphrase();

    #[derive(Deserialize)]
    struct WordpressUser {
      id: i32,
    }

    let auth = format!("Basic {}", base64::encode(format!("{}:{}", site.settings.wordpress.user, site.settings.wordpress.pass)));

    let user: WordpressUser = ureq::post(&format!("{}/wp/v2/users/", site.settings.wordpress.api_url))
      .set("Authorization", &auth)
      .send_json(serde_json::json!({
        "username": self.full_name,
        "password": &password,
        "email": self.email,
      }))?
      .into_json()?;

    sqlx::query!(
      "UPDATE students SET wordpress_user = $2, wordpress_initial_password = $3 WHERE id = $1",
      self.id,
      &user.id.to_string(),
      &password,
    ).execute(&site.db).await?;

    ureq::post(&format!("{}/ldlms/v2/users/{}/groups", site.settings.wordpress.api_url, user.id))
      .set("Authorization", &auth)
      .send_json(serde_json::json!({"group_ids":[site.settings.wordpress.student_group_id]}))?;

    self.wordpress_user = Some(user.id.to_string());
    self.wordpress_initial_password = Some(password);

    Ok(())

  }

  pub async fn send_payment_reminder(&self, site: &Site) -> Result<()> {
    let maybe_invoice = Invoice::query(
      InvoiceQuery{ student_id: Some(self.id), ..Default::default()}
    ).fetch_optional(&site.db).await?;

    match maybe_invoice {
      None => Ok(()),
      Some(invoice) => {
        let mut context = tera::Context::new();
        context.insert("full_name", &self.full_name);
        context.insert("checkout_link", &invoice.url);
        self.send_email(site, "Acerca de tu pago a DAO Education", "emails/payment_link", &context)
      }
    }
  }

  pub fn send_welcome_email(&mut self, site: &Site) -> Result<()> {
    let mut context = tera::Context::new();
    context.insert("full_name", &self.full_name);
    context.insert("email", &self.email);
    context.insert("password", &self.wordpress_initial_password);
    context.insert("discord_verification_link", &self.discord_verification_link(site));
    self.send_email(site, "Te damos la bienvenida a DAO Education", "emails/welcome", &context)
  }

  fn send_email(&self, site: &Site, subject: &str, template: &str, context: &tera::Context) -> Result<()> {
    let html = TEMPLATES.render(template, &context)?;

    ureq::post("https://api.sendinblue.com/v3/smtp/email")
      .set("api-key", &site.settings.sendinblue.api_key)
      .send_json(serde_json::json!({
        "sender": {
          "name": "DAO Education",
          "email": "dao.education@constata.eu",
        },
        "to": [{
          "email": &self.email,
          "name": &self.full_name,
        }],
        "replyTo":{"email":"info@dao.education"},
        "subject": subject,
        "htmlContent": html
      }))?;

    Ok(())
  }

  pub fn discord_verification_link(&self, site: &Site) -> Option<String> {
    self.discord_verification.as_ref().map(|token|{
      format!("https://discord.com/api/oauth2/authorize?response_type=token&client_id={}&state={}&scope=identify%20email%20guilds.join&redirect_uri={}/students/discord_success",
      site.settings.discord.client_id,
      &token,
      site.settings.checkout_domain,
    )})
  }

  pub async fn process_discord_response(site: &Site, discord: DiscordToken) -> Result<String> {
    let student = Student::find(&site, &StudentQuery{ discord_verification: Some(discord.state), ..Default::default()}).await?;
    let profile: DiscordProfile = ureq::get("https://discord.com/api/v9/users/@me")
      .set("Authorization", &format!("Bearer {}", discord.access_token))
      .call()?
      .into_json()?;

    let handle = format!("{}#{}", &profile.username, &profile.discriminator);

    let member_url = format!("https://discord.com/api/v9/guilds/{}/members/{}", site.settings.discord.guild_id, profile.id);

    let _ignored_because_it_may_be_member = ureq::request("PUT", &member_url)
      .set("Authorization", &format!("Bot {}", site.settings.discord.bot_secret_token))
      .send_json(serde_json::json![{"access_token": discord.access_token}])?;

    let _ignored_because_it_may_have_role = ureq::request("PUT", &format!("{}/roles/{}", &member_url, site.settings.discord.student_role_id))
      .set("Authorization", &format!("Bot {}", site.settings.discord.bot_secret_token))
      .send_json(serde_json::json![{}])?;

    sqlx::query!(
      "UPDATE students SET discord_handle = $2, discord_user_id = $3 WHERE id = $1",
      student.id,
      handle,
      profile.id
    ).execute(&site.db).await?;

    Ok(handle)
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
  pub async fn create(student_id: i32, plan: &Plan, site: &Site) -> Result<Subscription> {
    Ok(sqlx::query_as!(Subscription,
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
      student_id,
      plan.code.clone() as PlanCode,
      plan.signup
    ).fetch_one(&site.db).await?)
  }

  pub async fn create_monthly_charge(&self, site: &Site, today: &UtcDate) -> Result<MonthlyCharge> {
    Ok(sqlx::query_as!(MonthlyCharge,
      "INSERT INTO monthly_charges (student_id, subscription_id, price, billing_period)
        VALUES ($1, $2, $3, $4)
        RETURNING *
      ",
      self.student_id,
      self.id,
      site.settings.pricing.by_code(self.plan_code).monthly,
      today.and_hms(0,0,0),
    ).fetch_one(&site.db).await?)
  }

  pub async fn on_paid(&self, site: &Site) -> Result<()> {
    let mut student = Student::find_by_id(site, self.student_id).await?;
    student.setup_discord_verification(site).await?;
    student.setup_wordpress(site).await?;
    student.send_welcome_email(site)?;

    Ok(())
  }

  pub fn next_invoicing_date(&self) -> UtcDateTime {
    use chrono::prelude::*;
    let today = Utc::today();
    let this_months = Utc.ymd(today.year(), today.month(), self.invoicing_day as u32);
    let date = if today >= this_months {
      this_months + RelativeDuration::months(1)
    }else{
      this_months
    };
    date.and_hms(0,0,0)
  }
}

#[derive(Debug, Clone)]
pub struct MonthlyCharge {
  id: i32,
  created_at: UtcDateTime,
  billing_period: UtcDateTime,
  student_id: i32,
  subscription_id: i32,
  price: Decimal,
  paid: bool,
  paid_at: Option<UtcDateTime>,
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
  ) -> Result<Payment> {
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

    BillingSummary::new(site, Student::find_by_id(site, student_id).await?).await?.sync_paid_status().await?;

    Ok(payment)
  }

  pub async fn from_btcpay_webhook(webhook: &btcpay::Webhook, site: &Site) -> Result<Option<Payment>> {
    if webhook.kind != btcpay::WebhookType::InvoiceSettled {
      return Ok(None)
    }

    let maybe_invoice = Invoice::query(InvoiceQuery{
      external_id: Some(webhook.invoice_id.clone()),
      payment_method: Some(PaymentMethod::BtcPay),
      ..Default::default()
    }).fetch_optional(&site.db).await?;

    if let Some(invoice) = maybe_invoice {
      Ok(Some(invoice.make_payment(site, None).await?))
    } else {
      Ok(None)
    }
  }

  pub async fn from_invoice(site: &Site, invoice_id: i32) -> Result<Option<Payment>> {
    let maybe_invoice = Invoice::query(InvoiceQuery{
      id: Some(invoice_id),
      ..Default::default()
    }).fetch_optional(&site.db).await?;

    if let Some(invoice) = maybe_invoice {
      Ok(Some(invoice.make_payment(site, None).await?))
    } else {
      Ok(None)
    }
  }

  pub async fn from_stripe_event(e: &stripe::Event, site: &Site) -> Result<Option<Payment>> {
    use stripe::{EventType, EventObject};

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
        let maybe_invoice = Invoice::query(InvoiceQuery{
          amount: Some(amount),
          student_id: Some(student.id),
          payment_method: Some(PaymentMethod::Stripe),
          ..Default::default()
        }).fetch_optional(&site.db).await?;

        Ok(Some(Payment::create(
          site,
          student.id,
          amount,
          Decimal::new(i.tax.unwrap_or(0), 2),
          PaymentMethod::Stripe,
          &serde_json::to_string(&i)?,
          maybe_invoice.map(|i| i.id)
        ).await?))
      } else {
        Ok(None)
      }
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
  pub notified_on: Option<UtcDateTime>,
}

#[derive(Debug, Clone, Default)]
pub struct InvoiceQuery {
  pub id: Option<i32>,
  pub student_id: Option<i32>,
  pub external_id: Option<String>,
  pub amount: Option<Decimal>,
  pub payment_method: Option<PaymentMethod>,
  pub created_at: Option<UtcDateTime>,
  pub notified: Option<bool>,
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
        payment_id,
        notified_on
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
          ($6::boolean IS NULL OR (($6::boolean AND notified_on IS NOT NULL) OR (NOT $6::boolean AND notified_on IS NULL)))
          AND
          NOT paid AND NOT expired
        "#,
      query.id,
      query.student_id,
      query.external_id,
      query.amount,
      query.payment_method as Option<PaymentMethod>,
      query.notified,
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
        payment_id,
        notified_on
      "#,
      &student_id,
      amount,
      payment_method as PaymentMethod,
      description,
      url,
      external_id,
    ).fetch_one(&site.db).await
  }

  pub async fn make_payment(&self, site: &Site, clearing_data: Option<&str>) -> Result<Payment> {
    Payment::create(
      site,
      self.student_id,
      self.amount,
      Decimal::ZERO,
      self.payment_method,
      clearing_data.unwrap_or(""),
      Some(self.id)
    ).await
  }
}

#[rocket::async_trait]
pub trait BillingCharge: Send + Sync + std::fmt::Debug {
  fn description(&self) -> String;
  fn created_at(&self) -> UtcDateTime;
  fn amount(&self) -> Decimal;
  fn paid_at(&self) -> Option<UtcDateTime>;
  async fn set_paid(&mut self, site: &Site) -> Result<()>;
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

  async fn set_paid(&mut self, site: &Site) -> Result<()> {
    self.paid_at = Some(Utc::now());
    self.paid = true;
    sqlx::query!(
      "UPDATE monthly_charges SET paid = true, paid_at = $2 WHERE id = $1",
      self.id,
      self.paid_at,
    ).execute(&site.db).await?;
    Ok(())
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

  async fn set_paid(&mut self, site: &Site) -> Result<()> {
    self.paid_at = Some(Utc::now());
    self.paid = true;
    sqlx::query!(
      "UPDATE degrees SET paid = true, paid_at = $2 WHERE id = $1",
      self.id,
      self.paid_at,
    ).execute(&site.db).await?;
    Ok(())
  }
}

#[rocket::async_trait]
impl BillingCharge for Subscription {
  fn description(&self) -> String {
    "Subscripción".to_string()
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

  async fn set_paid(&mut self, site: &Site) -> Result<()> {
    self.paid_at = Some(Utc::now());
    self.paid = true;
    sqlx::query!(
      "UPDATE subscriptions SET paid = true, paid_at = $2 WHERE id = $1",
      self.id,
      self.paid_at,
    ).execute(&site.db).await?;
    self.on_paid(site).await?;
    Ok(())
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

#[derive(Serialize)]
pub struct BillingSummary<'a> {
  #[serde(skip_serializing)]
  pub student: Student,
  #[serde(skip_serializing)]
  pub site: &'a Site,

  pub subscription: Subscription,
  pub next_invoicing_date: UtcDateTime,
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
    let invoiceable = (balance * Decimal::NEGATIVE_ONE) - invoiced;

    let total_charges_not_invoiced_yet = if invoiceable.is_sign_positive() {
      Some( invoiceable )
    } else {
      None
    };

    let next_invoicing_date = subscription.next_invoicing_date();

    Ok(BillingSummary {
      subscription,
      student,
      site,
      history,
      unpaid_charges,
      invoices,
      total_charges_not_invoiced_yet,
      balance,
      next_invoicing_date,
    })
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

  /* Apply payments denormalizes the payment status from all outstanding charges
   * so that we know what to invoice. It may be the case that a customer payment
   * cannot cover the full of their debt so they need to top up again */
  pub async fn sync_paid_status(mut self) -> Result<()> {
    if self.unpaid_charges.is_empty() {
      return Ok(())
    }

    let mut unsynced: Decimal = self.unpaid_charges.iter().map(|c| c.amount() ).sum(); // - 120

    for charge in self.unpaid_charges.iter_mut() {
      if (unsynced - charge.amount()) * Decimal::NEGATIVE_ONE > self.balance {
        break;
      }

      charge.set_paid(&self.site).await?;
      unsynced -= charge.amount();
    }

    Ok(())
  }

  async fn request_on_stripe(&self) -> Result<Option<(String, String)>> {
    use serde_json::json;
    pub use stripe::{CheckoutSession, Subscription, ListSubscriptions, SubscriptionStatusFilter};

    let client = &self.site.stripe;
    let prices = self.site.settings.stripe_prices.by_plan_code(self.subscription.plan_code);
    let customer_id = self.student.get_or_create_stripe_customer_id(&client, &self.site).await?;

    let _subscribed = Subscription::list(client, ListSubscriptions{
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

    let invoice: btcpay::Invoice = ureq::post(&format!(
        "{}/api/v1/stores/{}/invoices",
        self.site.settings.btcpay.base_url,
        self.site.settings.btcpay.store_id,
      ))
      .set("Authorization", &format!("token {}", self.site.settings.btcpay.api_key))
      .send_json(serde_json::to_value(btcpay::InvoiceForm{ amount: total, currency: Currency::Eur })?)?
      .into_json()?;

    Ok(Some((invoice.checkout_link, invoice.id)))
  }

  pub async fn create_monthly_charges_for(&self, today: &UtcDate) -> Result<()> {
    use chrono::prelude::*;
    let month_start = Utc.ymd(today.year(), today.month(), 1);
    let month_end = month_start + RelativeDuration::months(1);
    let month_days = month_end.signed_duration_since(month_start).num_days() as i32;
    let this_day = today.day() as i32;
    let invoicing_day = self.subscription.invoicing_day;

    if invoicing_day == this_day || (invoicing_day > month_days && this_day == month_days) {
      let exists: bool = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT id FROM monthly_charges WHERE student_id = $1 AND billing_period = $2) as "exists!""#,
        self.student.id,
        today.and_hms(0,0,0),
      ).fetch_one(&self.site.db).await?;

      if exists {
        return Ok(());
      }

      self.subscription.create_monthly_charge(self.site, today).await?;
      self.invoice_all_not_invoiced_yet().await?;
    }
    Ok(())
  }
}

#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[sqlx(type_name = "payment_method", rename_all = "lowercase")]
pub enum PaymentMethod {
  Stripe,
  BtcPay,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Currency {
  Eur,
  Btc,
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

    let plans = SiteSettings::default().pricing;

    if latam.contains(&self.0.as_str()) {
      plans.latam
    } else if europe.contains(&self.0.as_str()) {
      plans.europe
    } else {
      plans.global
    }
  }
}

#[derive(sqlx::Type, PartialEq, Copy, Clone, Debug, Deserialize, Serialize)]
#[sqlx(type_name = "PlanCode")]
#[serde(rename_all = "lowercase")]
pub enum PlanCode {
  Global,
  Europe,
  Latam,
  Guest,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Plan {
  code: PlanCode,
  signup: Decimal,
  monthly: Decimal,
  degree: Decimal,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Plans {
  pub global: Plan,
  pub europe: Plan,
  pub latam: Plan,
  pub guest: Plan,
}

impl Plans {
  fn by_code(&self, code: PlanCode) -> Plan {
    match code {
      PlanCode::Global => self.global.clone(),
      PlanCode::Europe => self.europe.clone(),
      PlanCode::Latam  => self.latam.clone(),
      PlanCode::Guest  => self.guest.clone(),
    }
  }
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct DiscordSettings {
  pub guild_id: String,
  pub bot_secret_token: String,
  pub client_id: String,
  pub student_role_id: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct SendinblueSettings {
  pub api_key: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct WordpressSettings {
  pub api_url: String,
  pub user: String,
  pub pass: String,
  pub student_group_id: i32,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct BtcpaySettings {
  pub base_url: String,
  pub store_id: String,
  pub api_key: String,
  pub webhooks_secret: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct StripePrices {
  pub global_fzth_signup: PriceId,
  pub global_fzth_monthly: PriceId,
  pub global_fzth_degree: PriceId,
  pub latam_fzth_signup: PriceId,
  pub latam_fzth_monthly: PriceId,
  pub latam_fzth_degree: PriceId,
  pub europe_fzth_signup: PriceId,
  pub europe_fzth_monthly: PriceId,
  pub europe_fzth_degree: PriceId,
}

#[derive(Debug, PartialEq)]
pub struct StripePlanPrices<'a> {
  pub signup: &'a PriceId,
  pub monthly: &'a PriceId,
  pub degree: &'a PriceId,
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


pub struct AdminSession {
  pub token: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminSession {
  type Error = ();

  async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
    async fn build(req: &Request<'_>) -> Option<AdminSession> {
      let site = req.rocket().state::<Site>()?;
      let token_str = req.query_value::<&str>("admin_key").and_then(|r| r.ok())?;
      if token_str == site.settings.admin_key {
        Some(AdminSession{ token: token_str.to_string() })
      } else {
        None
      }
    }

    match build(req).await {
      Some(session) => Outcome::Success(session),
      None => Outcome::Failure((Status::Unauthorized, ())),
    }
  }
}

pub mod btcpay {
  use super::*;

  #[derive(Debug, PartialEq, Clone, Deserialize)]
  pub enum WebhookType {
    InvoiceCreated,
    InvoiceReceivedPayment,
    InvoicePaidInFull,
    InvoiceExpired,
    InvoiceSettled,
    InvoiceInvalid,
  }

  #[derive(Debug, PartialEq, Clone, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct Webhook {
    pub delivery_id: String,
    pub webhook_id: String,
    pub original_delivery_id: String,
    pub is_redelivery: bool,
    #[serde(rename = "type")]
    pub kind: WebhookType,
    pub timestamp: u64,
    pub store_id: String,
    pub invoice_id: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct Invoice {
    pub id: String,
    pub checkout_link: String,
  }

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct InvoiceForm {
    pub amount: Decimal,
    pub currency: Currency,
  }
}
