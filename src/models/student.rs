use crate::error::Result;
use super::*;

make_sqlx_model!{
  state: Site,
  table: students,
  Student {
    #[sqlx_search_as int4]
    id: i32,
    email: String,
    full_name: String,
    country: String,
    created_at: UtcDateTime,
    phone: Option<String>,
    tax_number: Option<String>,
    tax_address: Option<String>,
    referral_code: Option<String>,
    current_subscription_id: Option<i32>,
    wordpress_user: Option<String>,
    wordpress_initial_password: Option<String>,
    discord_user_id: Option<String>,
    discord_handle: Option<String>,
    #[sqlx_search_as varchar]
    discord_verification: Option<String>,
    #[sqlx_search_as varchar]
    stripe_customer_id: Option<String>,
    payment_method: PaymentMethod,
  }
}

impl NewStudent {
  pub async fn save_and_subscribe(self) -> Result<Student> {
    use chrono::Datelike;

    let tx = self.site.db.begin().await?;
    let student = self.save().await?;
    
    let plan = Country(student.attrs.country.clone()).plan();

    let subscription = student.site.subscription().build(NewSubscriptionAttrs{
      created_at: Utc::now(),
      invoicing_day: Utc::now().day() as i32,
      student_id: student.attrs.id,
      active: true,
      price: plan.signup,
      paid: false,
      plan_code: plan.code.clone(),
      paid_at: None,
      stripe_subscription_id: None,
    }).save().await?;

    subscription.create_monthly_charge(&Utc::today()).await?;
    tx.commit().await?;

    Ok(student)
  }
}

impl Student {
  pub async fn subscription(&self) -> sqlx::Result<Subscription> {
    self.site.subscription().find(&SubscriptionQuery{
      student_id_eq: Some(self.attrs.id),
      active_eq: Some(true),
      .. Default::default()
    }).await
  }

  pub fn discord_verification_link(&self) -> Option<String> {
    self.attrs.discord_verification.as_ref().map(|token|{
      format!("https://discord.com/api/oauth2/authorize?response_type=token&client_id={}&state={}&scope=identify%20email%20guilds.join&redirect_uri={}/students/discord_success",
      self.site.settings.discord.client_id,
      &token,
      self.site.settings.checkout_domain,
    )})
  }

  pub async fn setup_discord_verification(&mut self) -> Result<()> {
    let pass = gen_passphrase();
    sqlx::query!(
      "UPDATE students SET discord_verification = $2 WHERE id = $1",
      self.attrs.id,
      pass,
    ).execute(&self.site.db).await?;
    self.attrs.discord_verification = Some(pass);
    Ok(())
  }

  pub async fn get_or_create_stripe_customer_id(&self, client: &stripe::Client) -> Result<CustomerId> {
    use std::collections::HashMap;
    use stripe::CreateCustomer;

    if let Some(ref id) = self.attrs.stripe_customer_id {
      return Ok(id.parse::<CustomerId>()?);
    }

    let mut metadata = HashMap::new();

    metadata.insert("student_id".to_string(), self.attrs.id.to_string());
    let customer_id = Customer::create(client, CreateCustomer{
      email: Some(&self.attrs.email),
      metadata: Some(metadata), 
      ..Default::default()
    }).await?.id;

    sqlx::query!("UPDATE students SET stripe_customer_id = $1 WHERE id = $2",
      Some(customer_id.to_string()),
      self.attrs.id
    ).execute(&self.site.db).await?;
    Ok(customer_id)
  }

  pub async fn setup_wordpress(&mut self) -> Result<()> {
    if self.attrs.wordpress_user.is_some() {
      return Ok(())
    }

    let wp = &self.site.settings.wordpress;
    let auth = format!("Basic {}", base64::encode(format!("{}:{}", wp.user, wp.pass)));

    let password = gen_passphrase();

    #[derive(Deserialize)]
    struct WordpressUser {
      id: i32,
    }

    let user: WordpressUser = ureq::post(&format!("{}/wp/v2/users/", wp.api_url))
      .set("Authorization", &auth)
      .send_json(serde_json::json!({
        "username": self.attrs.full_name,
        "password": &password,
        "email": self.attrs.email,
      }))?
      .into_json()?;

    sqlx::query!(
      "UPDATE students SET wordpress_user = $2, wordpress_initial_password = $3 WHERE id = $1",
      self.attrs.id,
      &user.id.to_string(),
      &password,
    ).execute(&self.site.db).await?;

    ureq::post(&format!("{}/ldlms/v2/users/{}/groups", wp.api_url, user.id))
      .set("Authorization", &auth)
      .send_json(serde_json::json!({"group_ids":[wp.student_group_id]}))?;

    self.attrs.wordpress_user = Some(user.id.to_string());
    self.attrs.wordpress_initial_password = Some(password);

    Ok(())
  }

  pub async fn send_payment_reminder(&self) -> Result<()> {
    let maybe_invoice = self.site.invoice()
      .find_optional(&InvoiceQuery{ student_id_eq: Some(self.attrs.id), ..Default::default()})
      .await?;

    match maybe_invoice {
      None => Ok(()),
      Some(invoice) => {
        let mut context = tera::Context::new();
        context.insert("full_name", &self.attrs.full_name);
        context.insert("checkout_link", &invoice.attrs.url);
        self.send_email("Acerca de tu pago a DAO Education", "emails/payment_link", &context)
      }
    }
  }

  pub fn send_welcome_email(&mut self) -> Result<()> {
    let mut context = tera::Context::new();
    context.insert("full_name", &self.attrs.full_name);
    context.insert("email", &self.attrs.email);
    context.insert("password", &self.attrs.wordpress_initial_password);
    context.insert("discord_verification_link", &self.discord_verification_link());
    self.send_email("Te damos la bienvenida a DAO Education", "emails/welcome", &context)
  }

  fn send_email(&self, subject: &str, template: &str, context: &tera::Context) -> Result<()> {
    let html = TEMPLATES.render(template, &context)?;

    ureq::post("https://api.sendinblue.com/v3/smtp/email")
      .set("api-key", &self.site.settings.sendinblue.api_key)
      .send_json(serde_json::json!({
        "sender": {
          "name": "DAO Education",
          "email": "dao.education@constata.eu",
        },
        "to": [{
          "email": &self.attrs.email,
          "name": &self.attrs.full_name,
        }],
        "replyTo":{"email":"info@dao.education"},
        "subject": subject,
        "htmlContent": html
      }))?;

    Ok(())
  }
}

impl StudentHub {
  pub async fn process_discord_response(&self, discord: DiscordToken) -> Result<String> {
    let conf = &self.site.settings.discord;
    let student = self.find(&StudentQuery{
      discord_verification_eq: Some(Some(discord.state)),
      ..Default::default()}
    ).await?;
    let profile: DiscordProfile = ureq::get("https://discord.com/api/v9/users/@me")
      .set("Authorization", &format!("Bearer {}", discord.access_token))
      .call()?
      .into_json()?;

    let handle = format!("{}#{}", &profile.username, &profile.discriminator);

    let member_url = format!("https://discord.com/api/v9/guilds/{}/members/{}", conf.guild_id, profile.id);

    let _ignored_because_it_may_be_member = ureq::request("PUT", &member_url)
      .set("Authorization", &format!("Bot {}", conf.bot_secret_token))
      .send_json(serde_json::json![{"access_token": discord.access_token}])?;

    let _ignored_because_it_may_have_role =
      ureq::request("PUT", &format!("{}/roles/{}", &member_url, conf.student_role_id))
        .set("Authorization", &format!("Bot {}", conf.bot_secret_token))
        .send_json(serde_json::json![{}])?;

    sqlx::query!(
      "UPDATE students SET discord_handle = $2, discord_user_id = $3 WHERE id = $1",
      student.attrs.id,
      handle,
      profile.id
    ).execute(&self.site.db).await?;

    Ok(handle)
  }
}
