use serde::{Deserialize, Serialize};
use stripe::PriceId;

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Program {
  ZeroToHero,
  Academy,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Programs {
  pub zero_to_hero: PriceId,
  pub academy: PriceId,
}

impl Programs {
  pub fn price(&self, program: &Program) -> &PriceId {
    match program {
      Program::ZeroToHero => &self.zero_to_hero,
      Program::Academy => &self.academy,
    }
  }
}
