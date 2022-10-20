use std::fmt::Display;

use crate::{
    utils::statelist::{KeyHandler, StatefulList},
    MyKey,
};
use std::fmt;
impl Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ending = match &self.filter {
            true => "🗹",
            false => "⮽",
        };
        write!(f, "{} {}", self.name, ending)
    }
}

pub struct Item {
    pub name: String,
    pub filter: bool,
}

impl KeyHandler for Item {
    fn keyhandler(&mut self, key: MyKey) -> bool {
        if let MyKey::Enter | MyKey::Char(' ') = key {
            self.filter ^= true;
            return true;
        }
        false
    }
}

impl Item {
    fn new(name: String, filter: bool) -> Self {
        Self { name, filter }
    }
}

pub struct CheckBox {
    pub title: String,
    pub items: StatefulList<Item>,
}

impl CheckBox {
    pub fn new<T: Into<Vec<String>>>(title: String, items: T, filter: bool) -> Self {
        let strvec = items.into();
        let mut itemvec = vec![];
        for x in strvec {
            itemvec.push(Item::new(x.to_string(), filter));
        }
        let items = StatefulList::with_items(itemvec);
        Self { title, items }
    }
}
