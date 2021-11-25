use super::Deserialize;

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
