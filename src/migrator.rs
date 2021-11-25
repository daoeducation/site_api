use daoe_api::models::SiteSettings;

#[tokio::main]
async fn main() {
  sqlx::migrate!("src/migrations")
    .run(&SiteSettings::default().into_site().await.unwrap().db)
    .await
    .unwrap();
}
