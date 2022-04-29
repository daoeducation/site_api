use super::*;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Plan {
  pub code: PlanCode,
  pub signup: Decimal,
  pub degree: Decimal,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Plans {
  pub global: Plan,
  pub europe: Plan,
  pub latam: Plan,
  pub guest: Plan,
}

impl Plans {
  pub fn by_code(&self, code: PlanCode) -> Plan {
    match code {
      PlanCode::Global => self.global.clone(),
      PlanCode::Europe => self.europe.clone(),
      PlanCode::Latam  => self.latam.clone(),
      PlanCode::Guest  => self.guest.clone(),
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

