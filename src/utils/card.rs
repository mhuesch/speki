use crate::popups::edit_card::Editor;
use crate::popups::find_card::{CardPurpose, FindCardWidget};
use crate::popups::newchild::{AddChildWidget, Purpose};
use crate::utils::sql::fetch::fetch_card;
use crate::utils::sql::update::update_topic;
use crate::widgets::button::Button;
use crate::widgets::cardrater::CardRater;
use crate::widgets::textinput::Field;
use crate::widgets::topics::TopicList;
use crate::{MyKey, MyType};
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};
use tui::layout::Rect;
use tui::Frame;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum RecallGrade {
    None,
    Failed,
    Decent,
    Easy,
}

impl RecallGrade {
    pub fn from(num: u32) -> Option<Self> {
        match num {
            0 => Some(RecallGrade::None),
            1 => Some(RecallGrade::Failed),
            2 => Some(RecallGrade::Decent),
            3 => Some(RecallGrade::Easy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Review {
    pub grade: RecallGrade,
    pub date: UnixTime,
    pub answertime: f32,
}

impl Review {
    pub fn from(grade: &RecallGrade) -> Review {
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Review {
            grade: grade.clone(),
            date: unix,
            answertime: -1.,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CardType {
    Pending,
    Unfinished,
    Finished,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub id: CardID,
    pub question: String,
    pub answer: String,
    pub frontaudio: Option<PathBuf>,
    pub backaudio: Option<PathBuf>,
    pub frontimage: Option<PathBuf>,
    pub backimage: Option<PathBuf>,
    pub cardtype: CardTypeData,
    pub suspended: bool,
    pub resolved: bool,
    pub dependencies: Vec<IncID>,
    pub dependents: Vec<IncID>,
    pub history: Vec<Review>,
    pub topic: TopicID,
    pub source: IncID,
}

#[derive(Clone, Debug)]
pub enum CardTypeData {
    Finished(FinishedInfo),
    Unfinished(UnfinishedInfo),
    Pending(PendingInfo),
}

#[derive(Clone, Debug)]
pub struct FinishedInfo {
    pub strength: f32,
    pub stability: f32,
}

impl Default for FinishedInfo {
    fn default() -> Self {
        Self {
            strength: 1.0,
            stability: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UnfinishedInfo {
    pub skiptime: u32,
    pub skipduration: u32,
}

impl Default for UnfinishedInfo {
    fn default() -> Self {
        Self {
            skiptime: get_current_unix(),
            skipduration: 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PendingInfo {
    pub pos: u32,
}

impl Card {
    ///checks if the passed card should be resolved or not based on the completeness of its
    ///dependencies. If its status changed, it will recursively check all its dependents (and so
    ///on...)

    pub fn new(cardtype: CardTypeData) -> Card {
        Card {
            id: 0,
            question: String::new(),
            answer: String::new(),
            frontaudio: None,
            backaudio: None,
            frontimage: None,
            backimage: None,
            cardtype,
            suspended: false,
            resolved: false,
            dependencies: vec![],
            dependents: vec![],
            history: vec![],
            topic: 1,
            source: 0,
        }
    }

    pub fn question(mut self, question: String) -> Self {
        self.question = question;
        self
    }
    pub fn answer(mut self, answer: String) -> Self {
        self.answer = answer;
        self
    }
    pub fn source(mut self, source: IncID) -> Self {
        self.source = source;
        self
    }
    pub fn topic(mut self, topic: TopicID) -> Self {
        self.topic = topic;
        self
    }
    pub fn frontaudio(mut self, audiopath: Option<PathBuf>) -> Self {
        self.frontaudio = audiopath;
        self
    }
    pub fn backaudio(mut self, audiopath: Option<PathBuf>) -> Self {
        self.backaudio = audiopath;
        self
    }
    pub fn frontimage(mut self, imagepath: Option<PathBuf>) -> Self {
        self.frontimage = imagepath;
        self
    }
    pub fn backimage(mut self, imagepath: Option<PathBuf>) -> Self {
        self.backimage = imagepath;
        self
    }
    pub fn dependencies<IDVec: Into<Vec<CardID>>>(mut self, dependencies: IDVec) -> Self {
        for dependency in dependencies.into() {
            self.dependencies.push(dependency);
        }
        self
    }
    pub fn dependents<IDVec: Into<Vec<CardID>>>(mut self, dependents: IDVec) -> Self {
        for dependent in dependents.into() {
            self.dependents.push(dependent);
        }
        self
    }

    pub fn is_complete(&self) -> bool {
        if let CardTypeData::Finished(_) = self.cardtype {
            return true;
        }
        false
    }
    pub fn is_pending(&self) -> bool {
        if let CardTypeData::Pending(_) = self.cardtype {
            return true;
        }
        false
    }
    pub fn is_unfinished(&self) -> bool {
        if let CardTypeData::Unfinished(_) = self.cardtype {
            return true;
        }
        false
    }

    pub fn save_card(self, conn: &Arc<Mutex<Connection>>) -> CardID {
        let dependencies = self.dependencies.clone();
        let dependents = self.dependents.clone();
        let finished = self.is_complete();
        let card_id = save_card(conn, self);

        if finished {
            revlog_new(conn, card_id, &Review::from(&RecallGrade::Decent)).unwrap();
        }

        for dependency in dependencies {
            update_both(conn, card_id, dependency).unwrap();
        }
        for dependent in dependents {
            update_both(conn, dependent, card_id).unwrap();
            Self::check_resolved(dependent, conn);
        }

        Self::check_resolved(card_id, conn);
        card_id
    }

    pub fn check_resolved(id: u32, conn: &Arc<Mutex<Connection>>) -> bool {
        let mut change_detected = false;
        let mut card = fetch_card(conn, id);
        let mut is_resolved = true;

        for dependency in &card.dependencies {
            let dep_card = fetch_card(conn, *dependency);
            if !dep_card.resolved || !dep_card.is_complete() {
                is_resolved = false;
                break;
            };
        }
        if card.resolved != is_resolved {
            change_detected = true;
            card.resolved = is_resolved;
            set_resolved(conn, card.id, card.resolved);

            for dependent in card.dependents {
                Card::check_resolved(dependent, conn);
            }
        }
        change_detected
    }

    pub fn new_review(conn: &Arc<Mutex<Connection>>, id: CardID, review: RecallGrade) {
        revlog_new(conn, id, &Review::from(&review)).unwrap();
        super::interval::calc_stability(conn, id);
    }
    pub fn complete_card(conn: &Arc<Mutex<Connection>>, id: CardID) {
        let card = fetch_card(conn, id);
        remove_unfinished(conn, id).unwrap();
        new_finished(conn, id).unwrap();
        revlog_new(conn, id, &Review::from(&RecallGrade::Decent)).unwrap();
        for dependent in card.dependents {
            Card::check_resolved(dependent, conn);
        }
    }

    pub fn activate_card(conn: &Arc<Mutex<Connection>>, id: CardID) {
        remove_pending(conn, id).unwrap();
        new_finished(conn, id).unwrap();
    }

    pub fn play_frontaudio(appdata: &AppData, id: CardID) {
        let card = fetch_card(&appdata.conn, id);
        if let Some(path) = card.frontaudio {
            crate::utils::misc::play_audio(&appdata.audio, path);
        }
    }
    pub fn play_backaudio(appdata: &AppData, id: CardID) {
        let card = fetch_card(&appdata.conn, id);
        if let Some(path) = card.backaudio {
            crate::utils::misc::play_audio(&appdata.audio, path);
        }
    }
}

use super::misc::{get_current_unix, get_gpt3_response};
use super::sql::delete::{remove_pending, remove_unfinished};
use super::sql::fetch::fetch_question;
use super::sql::insert::new_finished;
use super::sql::insert::revlog_new;
use super::sql::update::{update_card_answer, update_card_question};
use super::sql::{
    insert::{save_card, update_both},
    update::set_resolved,
};
use super::statelist::{KeyHandler, StatefulList};
use crate::app::{AppData, TabData, Widget};
use crate::utils::aliases::*;

pub struct CardInfo {
    id: CardID,
    frontside: String,
    suspended: bool,
    resolved: bool,
    cardtype: CardType,
    strength: f32,
    stability: f32,
}

pub struct CardView<'a> {
    pub card: Option<Card>,
    pub revealed: bool,
    revealbutton: Button<'a>,
    pub cardrater: CardRater,
    pub question: Field,
    pub answer: Field,
    pub dependencies: StatefulList<CardItem>,
    pub dependents: StatefulList<CardItem>,
    pub topics: TopicList,
}

impl<'a> CardView<'a> {
    pub fn new(conn: &Arc<Mutex<Connection>>) -> Self {
        Self {
            card: None,
            revealed: true,
            revealbutton: Button::new("Reveal answer".to_string()),
            cardrater: CardRater::new(),
            question: Field::new("Question".to_string()),
            answer: Field::new("Answer".to_string()),
            dependencies: StatefulList::new("Dependencies".to_string()),
            dependents: StatefulList::new("Dependents".to_string()),
            topics: TopicList::new(conn),
        }
    }

    pub fn render(&mut self, f: &mut Frame<MyType>, appdata: &AppData, cursor: &Pos) {
        self.question.render(f, appdata, cursor);
        if self.revealed {
            self.answer.render(f, appdata, cursor);
            self.cardrater.render(f, appdata, cursor);
        } else {
            self.revealbutton.set_area(self.answer.get_area());
            self.revealbutton.render(f, appdata, cursor);
        }
        self.dependencies.render(f, appdata, cursor);
        self.dependents.render(f, appdata, cursor);
        self.topics.render(f, appdata, cursor);
    }

    pub fn keyhandler(
        &mut self,
        appdata: &AppData,
        tabdata: &mut TabData,
        cursor: &Pos,
        key: MyKey,
    ) {
        match key {
            MyKey::Char(' ') | MyKey::Enter | MyKey::KeyPress(_)
                if !self.revealed && self.answer.is_selected(cursor) =>
            {
                self.revealed = true;
                let area = self.cardrater.get_area();
                if area != Rect::default() {
                    tabdata.view.move_to_area(area);
                }
                if self.card.is_some() {
                    Card::play_backaudio(appdata, self.get_id());
                }
            }
            MyKey::Alt('g') if self.question.is_selected(cursor) && self.revealed => {
                if let Some(key) = &appdata.config.gptkey {
                    if let Some(answer) = get_gpt3_response(key, &self.question.return_text()) {
                        self.answer.replace_text(answer);
                    }
                }
            }
            MyKey::Alt('t') if self.card.is_some() => {
                let purpose = CardPurpose::NewDependent(vec![self.get_id()]);
                let cardfinder = FindCardWidget::new(&appdata.conn, purpose);
                tabdata.popup = Some(Box::new(cardfinder));
            }
            MyKey::Alt('y') if self.card.is_some() => {
                let purpose = CardPurpose::NewDependency(vec![self.get_id()]);
                let cardfinder = FindCardWidget::new(&appdata.conn, purpose);
                tabdata.popup = Some(Box::new(cardfinder));
            }
            MyKey::Alt('T') if self.card.is_some() => {
                let addchild =
                    AddChildWidget::new(appdata, Purpose::Dependency(vec![self.get_id()]));
                tabdata.popup = Some(Box::new(addchild));
            }
            MyKey::Alt('Y') if self.card.is_some() => {
                let addchild =
                    AddChildWidget::new(appdata, Purpose::Dependent(vec![self.get_id()]));
                tabdata.popup = Some(Box::new(addchild));
            }
            MyKey::Char('e') | MyKey::Enter if self.dependents.is_selected(cursor) => {
                if let Some(idx) = self.dependents.state.selected() {
                    let id = self.dependents.items[idx].id;
                    let editor = Editor::new(appdata, id);
                    tabdata.popup = Some(Box::new(editor));
                }
            }
            MyKey::Char('e') | MyKey::Enter if self.dependencies.is_selected(cursor) => {
                if let Some(idx) = self.dependencies.state.selected() {
                    let id = self.dependencies.items[idx].id;
                    let editor = Editor::new(appdata, id);
                    tabdata.popup = Some(Box::new(editor));
                }
            }
            key if self.question.is_selected(cursor) => self.question.keyhandler(appdata, key),
            key if self.revealed && self.answer.is_selected(cursor) => {
                self.answer.keyhandler(appdata, key)
            }
            key if self.dependencies.is_selected(cursor) => {
                self.dependencies.keyhandler(appdata, key)
            }
            key if self.dependents.is_selected(cursor) => self.dependents.keyhandler(appdata, key),
            key if self.cardrater.is_selected(cursor) => self.cardrater.keyhandler(appdata, key),
            key if self.topics.is_selected(cursor) => {
                self.topics.keyhandler(appdata, key);
                if let Some(topic_id) = self.topics.get_selected_id() {
                    if let Some(card) = &mut self.card {
                        card.topic = topic_id;
                    }
                }
            }
            _ => {}
        }
        self.save_state(&appdata.conn);
    }

    pub fn is_selected(&self, cursor: &Pos) -> bool {
        if self.question.is_selected(cursor) {
            return true;
        }
        if self.answer.is_selected(cursor) {
            return true;
        }
        if self.dependencies.is_selected(cursor) {
            return true;
        }
        if self.dependents.is_selected(cursor) {
            return true;
        }
        if self.cardrater.is_selected(cursor) {
            return true;
        }
        false
    }

    pub fn new_with_id(appdata: &AppData, id: CardID) -> Self {
        let mut myself = Self::new(&appdata.conn);
        myself.change_card(&appdata.conn, id);
        myself
    }

    pub fn change_card(&mut self, conn: &Arc<Mutex<Connection>>, id: CardID) {
        self.save_state(conn);
        let card = fetch_card(conn, id);
        let topic_id = card.topic;
        let idx = if topic_id == 0 {
            0
        } else {
            self.topics.index_from_id(topic_id) as usize
        };
        self.topics.state.select(Some(idx));

        self.question.replace_text(card.question.clone());
        self.answer.replace_text(card.answer.clone());
        self.dependencies = {
            let carditems = card
                .dependencies
                .clone()
                .into_iter()
                .map(|id| CardItem::from_id(conn, id))
                .collect();
            StatefulList::with_items("Dependencies".to_string(), carditems)
        };
        self.dependents = {
            let carditems = card
                .dependents
                .clone()
                .into_iter()
                .map(|id| CardItem::from_id(conn, id))
                .collect();
            StatefulList::with_items("Dependents".to_string(), carditems)
        };
        self.card = Some(card);
    }
    pub fn refresh(&mut self, appdata: &AppData) {
        if let Some(card) = &self.card {
            self.change_card(&appdata.conn, card.id);
        }
    }

    pub fn clear_card(&mut self, appdata: &AppData) {
        *self = Self::new(&appdata.conn);
    }

    pub fn save_state(&self, conn: &Arc<Mutex<Connection>>) {
        if self.card.is_none() {
            return;
        }
        let id = self.get_id();
        let topic_id = self.topics.get_selected_id().unwrap();
        update_card_question(conn, id, self.question.return_text());
        update_card_answer(conn, id, self.answer.return_text());
        update_topic(conn, id, topic_id);
    }

    pub fn get_id(&self) -> CardID {
        if let Some(card) = &self.card {
            return card.id as CardID;
        }
        panic!();
    }

    pub fn submit_card(&mut self, appdata: &AppData, iscompleted: bool) {
        let question = self.question.return_text();
        let answer = self.answer.return_text();
        let topic = self.topics.get_selected_id().unwrap();
        let source = 0;

        let status = if iscompleted {
            CardTypeData::Finished(FinishedInfo::default())
        } else {
            CardTypeData::Unfinished(UnfinishedInfo::default())
        };

        let card = Card::new(status)
            .question(question)
            .answer(answer)
            .topic(topic)
            .source(source);

        card.save_card(&appdata.conn);
    }
}

#[derive(Debug, Clone)]
pub struct CardItem {
    pub question: String,
    pub id: CardID,
}

impl CardItem {
    pub fn from_id(conn: &Arc<Mutex<Connection>>, id: CardID) -> Self {
        Self {
            question: fetch_question(conn, id),
            id,
        }
    }
}

impl KeyHandler for CardItem {}
use std::fmt::{self, Display};

impl Display for CardItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.question)
    }
}
