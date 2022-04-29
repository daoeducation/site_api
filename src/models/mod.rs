use sqlx_models_derive::make_sqlx_model;
use crate::TEMPLATES;
use crate::error::{Result, Error};
pub use chrono::{DateTime, Date, Utc};
pub use serde::{Deserialize, Serialize, ser::{Serializer, SerializeStruct}};
pub use sqlx::types::Decimal;
use validator::Validate;
pub use stripe::{PriceId, Price, Customer, CustomerId};
pub use rocket::{
  http::{uri::Path, Status},
  request::{FromRequest, Outcome, Request},
};
pub mod site;
pub use site::*;

pub mod student;
pub use student::*;

pub mod subscription;
pub use subscription::*;

pub mod degree;
pub use degree::*;

pub mod discord;
pub use discord::*;

pub mod plan;
pub use plan::*;

pub mod payment;
pub use payment::*;

pub mod invoice;
pub use invoice::*;

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
  pub fn into_insert_student(self, country: &Country) -> InsertStudent {
    InsertStudent{
      email: self.email,
      full_name: self.full_name,
      country: country.0.clone(),
      created_at: Utc::now(),
      phone: self.phone,
      tax_number: self.tax_number,
      tax_address: self.tax_address,
      referral_code: self.referral_code,
      current_subscription_id: None,
      wordpress_user: None,
      wordpress_initial_password: None,
      discord_user_id: None,
      discord_handle: None,
      discord_verification: None,
      stripe_customer_id: None,
      payment_method: self.payment_method,
    }
  }
}

#[derive(Serialize)]
pub struct StudentState {
  pub discord_verification_link: Option<String>,
  pub discord_handle: Option<String>,
  pub billing: BillingSummary,
}

impl StudentState {
  pub async fn new(student: Student) -> Result<StudentState> {
    Ok(Self{
      discord_verification_link: student.discord_verification_link(),
      discord_handle: student.attrs.discord_handle.clone(),
      billing: BillingSummary::new(student).await?,
    })
  }
}

#[rocket::async_trait]
pub trait BillingCharge: Send + Sync + std::fmt::Debug {
  fn description(&self) -> String;
  fn created_at(&self) -> UtcDateTime;
  fn amount(&self) -> Decimal;
  fn paid_at(&self) -> Option<UtcDateTime>;
  async fn set_paid(&mut self) -> Result<()>;
  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId;
}

impl Serialize for dyn BillingCharge {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("BillingHistoryItem", 4)?;
        state.serialize_field("created_at", &self.created_at())?;
        state.serialize_field("description", &self.description())?;
        state.serialize_field("amount", &self.amount())?;
        state.serialize_field("paid_at", &self.paid_at())?;
        state.end()
    }
}

#[rocket::async_trait]
impl BillingCharge for Degree {
  fn description(&self) -> String {
    "Titulación".to_string()
  }

  fn created_at(&self) -> UtcDateTime {
    self.attrs.created_at.clone()
  }

  fn amount(&self) -> Decimal {
    self.attrs.price.clone()
  }

  fn paid_at(&self) -> Option<UtcDateTime> {
    self.attrs.paid_at.clone()
  }

  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId {
    prices.degree
  }

  async fn set_paid(&mut self) -> Result<()> {
    self.attrs.paid_at = Some(Utc::now());
    self.attrs.paid = true;
    sqlx::query!(
      "UPDATE degrees SET paid = true, paid_at = $2 WHERE id = $1",
      self.attrs.id,
      self.attrs.paid_at,
    ).execute(&self.state.db).await?;
    Ok(())
  }
}

#[rocket::async_trait]
impl BillingCharge for Subscription {
  fn description(&self) -> String {
    "Subscripción".to_string()
  }

  fn created_at(&self) -> UtcDateTime {
    self.attrs.created_at.clone()
  }

  fn amount(&self) -> Decimal {
    self.attrs.price.clone()
  }

  fn paid_at(&self) -> Option<UtcDateTime> {
    self.attrs.paid_at.clone()
  }

  fn stripe_price<'a>(&self, prices: &'a StripePlanPrices) -> &'a PriceId {
    prices.signup
  }

  async fn set_paid(&mut self) -> Result<()> {
    self.attrs.paid_at = Some(Utc::now());
    self.attrs.paid = true;
    sqlx::query!(
      "UPDATE subscriptions SET paid = true, paid_at = $2 WHERE id = $1",
      self.attrs.id,
      self.attrs.paid_at,
    ).execute(&self.state.db).await?;
    self.on_paid().await?;
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
    self.attrs.created_at.clone()
  }

  fn description(&self) -> String {
    format!("Payment #{} via #{:?}", self.attrs.id, self.attrs.payment_method)
  }

  fn amount(&self) -> Decimal {
    self.attrs.amount
  }
}

#[derive(Serialize)]
pub struct BillingSummary {
  #[serde(skip_serializing)]
  pub student: student::Student,
  #[serde(skip_serializing)]
  pub state: Site,

  pub subscription: Subscription,
  pub history: Vec<Box<dyn BillingHistoryItem>>,
  pub unpaid_charges: Vec<Box<dyn BillingCharge>>,
  pub invoices: Vec<Invoice>,
  pub total_charges_not_invoiced_yet: Option<Decimal>,
  pub balance: Decimal,
}

impl BillingSummary {
  pub async fn new(student: student::Student) -> Result<BillingSummary> {
    let mut unpaid_charges: Vec<Box<dyn BillingCharge>> = vec![];
    let mut history: Vec<Box<dyn BillingHistoryItem>> = vec![];

    let site = &student.state;

    let subscription = student.subscription().await?;

    history.push(Box::new(subscription.clone()));

    if !subscription.attrs.paid {
      unpaid_charges.push(Box::new(subscription.clone()));
    }

    let degrees = site.degree().select().student_id_eq(student.id()).all().await?;

    for degree in degrees.into_iter() {
      if !degree.attrs.paid {
        unpaid_charges.push(Box::new(degree.clone()));
      }
      history.push(Box::new(degree));
    }

    let payments = site.payment().select().student_id_eq(student.id()).all().await?;

    for payment in payments.into_iter() {
      history.push(Box::new(payment))
    }

    let balance: Decimal = history.iter().map(|i| i.amount() ).sum();
    let invoices = site.invoice().select()
      .student_id_eq(student.id())
      .paid_eq(&false)
      .expired_eq(&false)
      .all().await?;
    let invoiced: Decimal = invoices.iter().map(|i| i.attrs.amount ).sum();
    let invoiceable = (balance * Decimal::NEGATIVE_ONE) - invoiced;

    let total_charges_not_invoiced_yet = if invoiceable.is_sign_positive() {
      Some( invoiceable )
    } else {
      None
    };

    Ok(BillingSummary {
      state: student.state.clone(),
      subscription,
      student,
      history,
      unpaid_charges,
      invoices,
      total_charges_not_invoiced_yet,
      balance,
    })
  }

  pub async fn invoice_all_not_invoiced_yet(&self) -> Result<Option<Invoice>> {
    if self.subscription.attrs.plan_code == PlanCode::Guest {
      return Ok(None)
    }

    let amount = match self.total_charges_not_invoiced_yet {
      Some(a) => a,
      None => return Ok(None),
    };

    let maybe_url_and_external_id = match self.student.attrs.payment_method {
      PaymentMethod::Stripe => self.request_on_stripe().await?,
      PaymentMethod::BtcPay => self.request_on_btcpay().await?,
    };

    match maybe_url_and_external_id {
      Some((url, external_id)) => Ok(Some(self.state.invoice().insert().use_struct(InsertInvoice{
        student_id: self.student.attrs.id,
        created_at: Utc::now(),
        payment_method: self.student.attrs.payment_method,
        external_id: external_id,
        amount: amount,
        description: "Cargos pendientes".to_string(),
        url: url,
        paid: false,
        expired: false,
        payment_id: None,
        notified_on: None,
      }).save().await?)),
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

      charge.set_paid().await?;
      unsynced -= charge.amount();
    }

    Ok(())
  }

  async fn request_on_stripe(&self) -> Result<Option<(String, String)>> {
    use serde_json::json;
    pub use stripe::{CheckoutSession, Subscription, ListSubscriptions, SubscriptionStatusFilter};

    let client = &self.state.stripe;
    let prices = self.state.settings.stripe_prices.by_plan_code(self.subscription.attrs.plan_code);
    let customer_id: CustomerId = self.student.get_or_create_stripe_customer_id(&client).await?;

    let _subscribed = Subscription::list(client, ListSubscriptions{
      customer: Some(customer_id.clone()),
      status: Some(SubscriptionStatusFilter::Active),
      ..ListSubscriptions::new()
    }).await?.total_count.unwrap_or(0) > 0;

    let line_items: Vec<&PriceId> = self.unpaid_charges.iter()
      .map(|i| i.stripe_price(&prices) ).collect();

    let stripe_session : CheckoutSession = client.post_form("/checkout/sessions", json![{
      "success_url": self.state.settings.payment_success_redirect.clone(),
      "cancel_url": self.state.settings.payment_error_redirect.clone(),
      "customer": customer_id,
      "payment_method_types": ["card"],
      "mode": "payment",
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
        self.state.settings.btcpay.base_url,
        self.state.settings.btcpay.store_id,
      ))
      .set("Authorization", &format!("token {}", self.state.settings.btcpay.api_key))
      .send_json(serde_json::to_value(btcpay::InvoiceForm{
        amount: total,
        currency: Currency::Eur,
        checkout: btcpay::InvoiceFormCheckout{ redirectURL: self.state.settings.payment_success_redirect.clone() }
      })?)?
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
  pub global_fzth_degree: PriceId,
  pub latam_fzth_signup: PriceId,
  pub latam_fzth_degree: PriceId,
  pub europe_fzth_signup: PriceId,
  pub europe_fzth_degree: PriceId,
}

#[derive(Debug, PartialEq)]
pub struct StripePlanPrices<'a> {
  pub signup: &'a PriceId,
  pub degree: &'a PriceId,
}

impl StripePrices {
  pub async fn validate_all(&self, client: &stripe::Client) -> Result<()> {
    let prices = vec![
      &self.global_fzth_signup,
      &self.global_fzth_degree,
      &self.latam_fzth_signup,
      &self.latam_fzth_degree,
      &self.europe_fzth_signup,
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
        degree: &self.europe_fzth_degree,
      },
      PlanCode::Latam => StripePlanPrices{
        signup: &self.latam_fzth_signup,
        degree: &self.latam_fzth_degree,
      },
      _ => StripePlanPrices {
        signup: &self.global_fzth_signup,
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
  pub struct InvoiceFormCheckout {
    pub redirectURL: String,
  }

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct InvoiceForm {
    pub amount: Decimal,
    pub currency: Currency,
    pub checkout: InvoiceFormCheckout
  }
}
