use std::fmt;
use std::fmt::Display;

use crate::utils::aliases::*;
use crate::utils::statelist::KeyHandler;

#[derive(Debug, Clone)]
pub struct CardItem {
    pub question: String,
    pub id: CardID,
}

impl KeyHandler for CardItem {}

impl Display for CardItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.question)
    }
}
