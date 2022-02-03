use super::Deserialize;

#[derive(Debug, FromForm)]
pub struct DiscordToken {
  pub state: String,
  pub access_token: String,
}

#[derive(Deserialize)]
pub struct DiscordProfile {
  pub id: String,
  pub username: String,
  pub discriminator: String,
  pub verified: bool,
  pub email: String,
}
