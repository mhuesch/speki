use crate::app::{AppData, PopUp, Widget};
use crate::utils::aliases::*;
use crate::utils::misc::{
    get_dependencies, get_dependents, get_gpt3_response, split_leftright_by_percent, split_updown,
    split_updown_by_percent, View,
};
use crate::utils::sql::update::{set_suspended, update_inc_active};
use crate::widgets::button::Button;
use crate::widgets::mode_status::ModeStatus;
use crate::widgets::progress_bar::ProgressBar;
use crate::widgets::textinput::Field;
use crate::widgets::{
    find_card::{CardPurpose, FindCardWidget},
    newchild::{AddChildWidget, Purpose},
};
use crate::{
    app::Tab,
    utils::{
        card::{Card, CardType, RecallGrade},
        misc::modecolor,
        sql::{
            fetch::get_cardtype,
            update::{double_inc_skip_duration, double_skip_duration},
        },
    },
    MyType,
};
use rand::prelude::*;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

pub enum ReviewMode {
    Review(CardReview),
    Pending(CardReview),
    Unfinished(UnfCard),
    IncRead(IncMode),
    Done,
}

pub struct ForReview {
    pub review_cards: Vec<CardID>,
    pub unfinished_cards: Vec<CardID>,
    pub pending_cards: Vec<CardID>,
    pub active_increads: Vec<IncID>,
}

impl ForReview {
    pub fn new(conn: &Arc<Mutex<Connection>>) -> Self {
        crate::utils::interval::calc_strength(conn);

        let mut review_cards = CardQuery::default()
            .cardtype(vec![CardType::Finished])
            .strength((0., 0.9))
            .suspended(false)
            .resolved(true)
            .fetch_card_ids(conn);

        let mut unfinished_cards = CardQuery::default()
            .unfinished_due()
            .suspended(false)
            .resolved(true)
            .fetch_card_ids(conn);

        let pending_cards = CardQuery::default()
            .cardtype(vec![CardType::Pending])
            .order_by("ORDER BY position DESC".to_string())
            .suspended(false)
            .resolved(true)
            .fetch_card_ids(conn);

        let active_increads = load_active_inc(conn).unwrap();
        unfinished_cards.shuffle(&mut thread_rng());
        review_cards.shuffle(&mut thread_rng());

        ForReview {
            review_cards,
            unfinished_cards,
            pending_cards,
            active_increads,
        }
    }
}

pub struct StartQty {
    pub fin_qty: u16,
    pub unf_qty: u16,
    pub pending_qty: u16,
    pub inc_qty: u16,
}

impl StartQty {
    pub fn new(for_review: &ForReview) -> Self {
        let fin_qty = for_review.review_cards.len() as u16;
        let unf_qty = for_review.unfinished_cards.len() as u16;
        let pending_qty = for_review.pending_cards.len() as u16;
        let inc_qty = for_review.active_increads.len() as u16;

        StartQty {
            fin_qty,
            unf_qty,
            pending_qty,
            inc_qty,
        }
    }
}

pub struct MainReview {
    progress_bar: ProgressBar,
    pub mode: ReviewMode,
    pub status: ModeStatus,
    pub for_review: ForReview,
    pub start_qty: StartQty,
    pub automode: bool,
    pub popup: Option<Box<dyn PopUp>>,
    view: View,
}

use crate::utils::sql::fetch::{load_active_inc, CardQuery};

impl MainReview {
    pub fn new(appdata: &AppData) -> Self {
        let mode = ReviewMode::Done;
        let for_review = ForReview::new(&appdata.conn);
        let start_qty = StartQty::new(&for_review);
        let progress_bar = ProgressBar::new("Progress".to_string());
        let status = ModeStatus::default();

        let mut myself = Self {
            progress_bar,
            mode,
            status,
            for_review,
            start_qty,
            automode: true,
            popup: None,
            view: View::default(),
        };
        myself.random_mode(appdata);
        myself
    }

    fn update_dependencies(&mut self, conn: &Arc<Mutex<Connection>>) {
        match &mut self.mode {
            ReviewMode::Review(rev) => {
                rev.cardview.dependencies = get_dependencies(conn, rev.cardview.get_id());
                rev.cardview.dependents = get_dependents(conn, rev.cardview.get_id());
            }
            ReviewMode::Unfinished(rev) => {
                rev.cardview.dependencies = get_dependencies(conn, rev.cardview.get_id());
                rev.cardview.dependents = get_dependents(conn, rev.cardview.get_id());
            }
            ReviewMode::Pending(rev) => {
                rev.cardview.dependencies = get_dependencies(conn, rev.cardview.get_id());
                rev.cardview.dependents = get_dependents(conn, rev.cardview.get_id());
            }
            _ => {}
        }
    }

    pub fn draw_done(&mut self, f: &mut Frame<crate::MyType>, appdata: &AppData, area: Rect) {
        let mut field = Field::default();
        let mut button = Button::new("Nothing left to review now!\n\nYou could import anki cards from the import page, or add new cards manually.\n\nIf you've imported cards, press Alt+r here to refresh".to_string());
        field.set_area(area);
        let cursor = &self.get_cursor();
        button.render(f, appdata, cursor)
    }

    #[allow(clippy::single_match)]
    pub fn mode_done(&mut self, appdata: &AppData, key: MyKey) {
        match key {
            MyKey::Alt('r') => *self = crate::tabs::review::logic::MainReview::new(appdata),
            _ => {}
        }
    }
    // randomly choose a mode between active, unfinished and inc read, if theyre all done,
    // start with pending cards, if theyre all done, declare nothing left to review
    pub fn random_mode(&mut self, appdata: &AppData) {
        let act: u32 = self.for_review.review_cards.len() as u32;
        let unf: u32 = self.for_review.unfinished_cards.len() as u32 + act;
        let inc: u32 = self.for_review.active_increads.len() as u32 + unf;

        let pending_qty = self.for_review.pending_cards.len() as u32;
        if inc == 0 {
            if pending_qty > 0 {
                self.new_pending_mode(appdata);
            } else {
                self.mode = ReviewMode::Done;
            }
            return;
        }

        let mut rng = rand::thread_rng();
        let rand = rng.gen_range(0..inc);

        if rand < act {
            self.new_review_mode(appdata);
        } else if rand < unf {
            self.new_unfinished_mode(appdata);
        } else if rand < inc {
            self.new_inc_mode(appdata);
        } else {
            panic!();
        };
    }

    pub fn new_inc_mode(&mut self, appdata: &AppData) {
        let id = self.for_review.active_increads.remove(0);
        let inc = IncMode::new(appdata, id);
        self.mode = ReviewMode::IncRead(inc);
    }

    pub fn new_unfinished_mode(&mut self, appdata: &AppData) {
        let id = self.for_review.unfinished_cards.remove(0);
        let unfcard = UnfCard::new(appdata, id);
        self.mode = ReviewMode::Unfinished(unfcard);
        Card::play_frontaudio(appdata, id);
    }

    pub fn new_pending_mode(&mut self, appdata: &AppData) {
        let id = self.for_review.pending_cards.remove(0);
        let cardreview = CardReview::new(id, appdata);
        self.mode = ReviewMode::Pending(cardreview);
    }

    pub fn new_review_mode(&mut self, appdata: &AppData) {
        let id = self.for_review.review_cards.remove(0);
        let cardreview = CardReview::new(id, appdata);
        self.mode = ReviewMode::Review(cardreview);
    }

    pub fn inc_next(&mut self, appdata: &AppData, id: IncID) {
        self.random_mode(appdata);
        double_inc_skip_duration(&appdata.conn, id).unwrap();
    }
    pub fn inc_done(&mut self, appdata: &AppData, id: IncID) {
        let active = false;
        update_inc_active(&appdata.conn, id, active).unwrap();
        self.random_mode(appdata);
    }

    pub fn new_review(&mut self, appdata: &AppData, id: CardID, recallgrade: RecallGrade) {
        Card::new_review(&appdata.conn, id, recallgrade);
        self.random_mode(appdata);
    }

    pub fn draw_progress_bar(&mut self, f: &mut Frame<MyType>, appdata: &AppData, _area: Rect) {
        let target = match self.mode {
            ReviewMode::Done => return,
            ReviewMode::Review(_) => self.start_qty.fin_qty,
            ReviewMode::Pending(_) => self.start_qty.pending_qty,
            ReviewMode::IncRead(_) => self.start_qty.inc_qty,
            ReviewMode::Unfinished(_) => self.start_qty.unf_qty,
        } as u32;

        let current = match self.mode {
            ReviewMode::Done => 0,
            ReviewMode::Review(_) => {
                (self.start_qty.fin_qty as u32) - (self.for_review.review_cards.len() as u32)
            }
            ReviewMode::Pending(_) => {
                (self.start_qty.pending_qty as u32) - (self.for_review.pending_cards.len() as u32)
            }
            ReviewMode::IncRead(_) => {
                (self.start_qty.inc_qty as u32) - (self.for_review.active_increads.len() as u32)
            }
            ReviewMode::Unfinished(_) => {
                (self.start_qty.unf_qty as u32) - (self.for_review.unfinished_cards.len() as u32)
            }
        };

        let color = modecolor(&self.mode);
        let cursor = self.get_cursor();
        self.progress_bar.current = current;
        self.progress_bar.max = target;
        self.progress_bar.color = color;
        self.progress_bar.render(f, appdata, &cursor);
    }
}

impl Tab for MainReview {
    fn set_selection(&mut self, area: Rect) {
        self.view.areas.clear();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(4), Constraint::Ratio(7, 10)].as_ref())
            .split(area);

        let (progbar, area) = (chunks[0], chunks[1]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Ratio(7, 10)].as_ref())
            .split(progbar);

        let (status, progbar) = (chunks[0], chunks[1]);
        self.status.set_area(status);
        self.progress_bar.set_area(progbar);
        self.view.areas.push(status);
        self.view.areas.push(progbar);

        match &mut self.mode {
            ReviewMode::Review(rev) | ReviewMode::Pending(rev) => {
                let updown = split_updown([Constraint::Ratio(9, 10), Constraint::Min(5)], area);
                let (up, down) = (updown[0], updown[1]);
                let leftright = split_leftright_by_percent([66, 33], up);
                let bottomleftright = split_leftright_by_percent([66, 33], down);
                let left = leftright[0];
                let right = leftright[1];
                let rightcolumn = split_updown_by_percent([50, 50], right);
                let leftcolumn = split_updown_by_percent([50, 50], left);

                self.view.areas.push(leftcolumn[0]);
                self.view.areas.push(leftcolumn[1]);
                self.view.areas.push(rightcolumn[0]);
                self.view.areas.push(rightcolumn[1]);
                self.view.areas.push(bottomleftright[0]);
                self.view.areas.push(bottomleftright[1]);

                if rev.cardview.question.get_area().width == 0 {
                    self.view.move_to_area(leftcolumn[0]);
                }

                rev.cardview.question.set_area(leftcolumn[0]);
                rev.cardview.answer.set_area(leftcolumn[1]);
                rev.cardview.dependents.set_area(rightcolumn[0]);
                rev.cardview.dependencies.set_area(rightcolumn[1]);
                rev.cardview.cardrater.set_area(bottomleftright[0]);
            }
            ReviewMode::Unfinished(unf) => {
                let leftright = split_leftright_by_percent([66, 33], area);
                let left = leftright[0];
                let right = leftright[1];

                let rightcolumn = split_updown_by_percent([50, 50], right);
                let leftcolumn = split_updown_by_percent([50, 50], left);

                self.view.areas.push(leftcolumn[0]);
                self.view.areas.push(leftcolumn[1]);
                self.view.areas.push(rightcolumn[0]);
                self.view.areas.push(rightcolumn[1]);

                unf.cardview.question.set_area(leftcolumn[0]);
                unf.cardview.answer.set_area(leftcolumn[1]);
                unf.cardview.dependents.set_area(rightcolumn[0]);
                unf.cardview.dependencies.set_area(rightcolumn[1]);
            }
            ReviewMode::IncRead(rev) => {
                let mainvec = split_leftright_by_percent([75, 15], area);
                let (editing, rightside) = (mainvec[0], mainvec[1]);
                let rightvec = split_updown_by_percent([10, 40, 40], rightside);

                self.view.areas.push(editing);
                self.view.areas.push(rightvec[0]);
                self.view.areas.push(rightvec[1]);
                self.view.areas.push(rightvec[2]);

                rev.source.source.set_area(editing);
                rev.source.extracts.set_area(rightvec[1]);
                rev.source.clozes.set_area(rightvec[2]);
            }
            ReviewMode::Done => {}
        }
    }
    fn get_cursor(&self) -> (u16, u16) {
        self.view.cursor
    }
    fn navigate(&mut self, dir: crate::NavDir) {
        if let Some(popup) = &mut self.popup {
            popup.navigate(dir);
        } else {
            self.view.navigate(dir);
        }
    }
    fn get_title(&self) -> String {
        "Review".to_string()
    }

    fn get_manual(&self) -> String {
        match &self.mode {
            ReviewMode::Done => "".to_string(),
            ReviewMode::Review(rev) => rev.get_manual(),
            ReviewMode::Pending(rev) => rev.get_manual(),
            ReviewMode::IncRead(inc) => inc.get_manual(),
            ReviewMode::Unfinished(unf) => unf.get_manual(),
        }
    }

    fn render(&mut self, f: &mut Frame<crate::MyType>, appdata: &AppData, area: Rect) {
        self.set_selection(area);
        let cursor = &self.get_cursor();

        self.status
            .render_it(f, &self.mode, &self.for_review, &self.start_qty);
        self.draw_progress_bar(f, appdata, area);

        match &mut self.mode {
            ReviewMode::Done => self.draw_done(f, appdata, area),
            ReviewMode::Review(review) => review.render(f, appdata, cursor),
            ReviewMode::Pending(pending) => pending.render(f, appdata, cursor),
            ReviewMode::Unfinished(unfinished) => unfinished.render(f, appdata, cursor),
            ReviewMode::IncRead(inc) => inc.render(f, appdata, cursor),
        }

        if let Some(popup) = &mut self.popup {
            if popup.should_quit() {
                self.popup = None;
                self.update_dependencies(&appdata.conn);
                return;
            }
            popup.render_popup(f, appdata, area);
        }
    }

    fn keyhandler(&mut self, appdata: &AppData, key: MyKey) {
        let cursor = &self.get_cursor();
        use MyKey::*;
        if let Some(popup) = &mut self.popup {
            popup.keyhandler(appdata, key);
            return;
        }

        match &mut self.mode {
            ReviewMode::Done => self.mode_done(appdata, key),
            ReviewMode::Unfinished(unf) => match key {
                Alt('s') => {
                    let id = unf.cardview.get_id();
                    unf.cardview.save_state(&appdata.conn);
                    self.random_mode(appdata);
                    double_skip_duration(&appdata.conn, id);
                }
                Alt('f') => {
                    let id = unf.cardview.get_id();
                    unf.cardview.save_state(&appdata.conn);
                    Card::complete_card(&appdata.conn, id);
                    self.random_mode(appdata);
                }
                Alt('t') => {
                    let id = unf.cardview.get_id();
                    let purpose = CardPurpose::NewDependent(vec![id]);
                    let cardfinder = FindCardWidget::new(&appdata.conn, purpose);
                    self.popup = Some(Box::new(cardfinder));
                }
                Alt('y') => {
                    let id = unf.cardview.get_id();
                    let purpose = CardPurpose::NewDependency(vec![id]);
                    let cardfinder = FindCardWidget::new(&appdata.conn, purpose);
                    self.popup = Some(Box::new(cardfinder));
                }
                Alt('T') => {
                    let id = unf.cardview.get_id();
                    let addchild =
                        AddChildWidget::new(&appdata.conn, Purpose::Dependency(vec![id]));
                    self.popup = Some(Box::new(addchild));
                }
                Alt('Y') => {
                    let id = unf.cardview.get_id();
                    let addchild = AddChildWidget::new(&appdata.conn, Purpose::Dependent(vec![id]));
                    self.popup = Some(Box::new(addchild));
                }
                Alt('g') => {
                    if let Some(key) = &appdata.config.gptkey {
                        let answer = get_gpt3_response(key, &unf.cardview.question.return_text());
                        unf.cardview.answer.replace_text(answer);
                    }
                }
                Alt('i') => {
                    let id = unf.cardview.get_id();
                    set_suspended(&appdata.conn, [id], true);
                    unf.cardview.save_state(&appdata.conn);
                    self.random_mode(appdata);
                }
                key if unf.cardview.question.is_selected(cursor) => {
                    unf.cardview.question.keyhandler(appdata, key)
                }
                key if unf.cardview.answer.is_selected(cursor) => {
                    unf.cardview.answer.keyhandler(appdata, key)
                }
                _ => {}
            },
            ReviewMode::Pending(rev) | ReviewMode::Review(rev) => match key {
                KeyPress(pos) => self.view.cursor = pos,
                Alt('s') => {
                    rev.cardview.save_state(&appdata.conn);
                    self.random_mode(appdata);
                }
                Alt('t') => {
                    let purpose = CardPurpose::NewDependent(vec![rev.cardview.get_id()]);
                    let cardfinder = FindCardWidget::new(&appdata.conn, purpose);
                    self.popup = Some(Box::new(cardfinder));
                }
                Alt('y') => {
                    let purpose = CardPurpose::NewDependency(vec![rev.cardview.get_id()]);
                    let cardfinder = FindCardWidget::new(&appdata.conn, purpose);
                    self.popup = Some(Box::new(cardfinder));
                }
                Alt('T') => {
                    let addchild = AddChildWidget::new(
                        &appdata.conn,
                        Purpose::Dependency(vec![rev.cardview.get_id()]),
                    );
                    self.popup = Some(Box::new(addchild));
                }
                Alt('Y') => {
                    let addchild = AddChildWidget::new(
                        &appdata.conn,
                        Purpose::Dependent(vec![rev.cardview.get_id()]),
                    );
                    self.popup = Some(Box::new(addchild));
                }
                Alt('i') => {
                    set_suspended(&appdata.conn, [rev.cardview.get_id()], true);
                    rev.cardview.save_state(&appdata.conn);
                    self.random_mode(appdata);
                }
                Char(' ') | Enter if rev.cardview.answer.is_selected(cursor) => {
                    rev.cardview.revealed = true;
                    let area = rev.cardview.cardrater.get_area();
                    self.view.move_to_area(area);
                    Card::play_backaudio(appdata, rev.cardview.get_id());
                }

                Char(num)
                    if rev.cardview.cardrater.is_selected(cursor)
                        && num.is_ascii_digit()
                        && (1..5).contains(&num.to_digit(10).unwrap()) =>
                {
                    let id = rev.cardview.get_id();
                    let grade = match num {
                        '1' => RecallGrade::None,
                        '2' => RecallGrade::Failed,
                        '3' => RecallGrade::Decent,
                        '4' => RecallGrade::Easy,
                        _ => panic!("illegal argument"),
                    };
                    if get_cardtype(&appdata.conn, id) == CardType::Pending {
                        Card::activate_card(&appdata.conn, id);
                    }
                    rev.cardview.save_state(&appdata.conn);
                    self.new_review(appdata, id, grade);
                }
                Char(' ') | Enter
                    if rev.cardview.cardrater.is_selected(cursor)
                        && rev.cardview.cardrater.selection.is_some() =>
                {
                    let grade = rev.cardview.cardrater.selection.clone().unwrap();
                    let id = rev.cardview.get_id();
                    if get_cardtype(&appdata.conn, id) == CardType::Pending {
                        Card::activate_card(&appdata.conn, id);
                    }
                    rev.cardview.save_state(&appdata.conn);
                    self.new_review(appdata, id, grade);
                }
                key if rev.cardview.is_selected(cursor) => {
                    rev.cardview.keyhandler(appdata, cursor, key);
                }
                _ => {}
            },
            ReviewMode::IncRead(inc) => match key {
                KeyPress(pos) => self.view.cursor = pos,
                Alt('d') => {
                    inc.source.update_text(&appdata.conn);
                    let id = inc.source.id;
                    self.inc_done(appdata, id);
                }
                Alt('s') => {
                    inc.source.update_text(&appdata.conn);
                    let id = inc.source.id;
                    self.inc_next(appdata, id);
                }
                Alt('a') if inc.source.source.is_selected(cursor) => {
                    let addchild =
                        AddChildWidget::new(&appdata.conn, Purpose::Source(inc.source.id));
                    self.popup = Some(Box::new(addchild));
                }
                key if inc.source.extracts.is_selected(cursor) => {
                    inc.source.extracts.keyhandler(appdata, key)
                }
                key if inc.source.clozes.is_selected(cursor) => {
                    inc.source.clozes.keyhandler(appdata, key)
                }
                key if inc.source.source.is_selected(cursor) => inc.source.keyhandler(appdata, key),
                _ => {}
            },
        }
    }
}
use crate::MyKey;

use super::reviewmodes::finished::CardReview;
use super::reviewmodes::incread::IncMode;
use super::reviewmodes::unfinished::UnfCard;
