//! The first-run setup wizard: welcome, about you, and ready to start.
//!
//! Steps are listed in `Step::ALL`; to add one (for example connecting a bank later),
//! add a variant there, give it a title, and draw it in `Wizard::show`.

use super::widgets::*;
use super::*;
use eframe::egui::pos2;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Step {
    Welcome,
    AboutYou,
    Ready,
}

impl Step {
    const ALL: [Step; 3] = [Step::Welcome, Step::AboutYou, Step::Ready];

    fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn title(self) -> &'static str {
        match self {
            Step::Welcome => "Welcome",
            Step::AboutYou => "About you",
            Step::Ready => "Ready",
        }
    }
}

/// How to fill the app once setup is done.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum StartWith {
    SampleData,
    CleanSlate,
    /// Only offered when running setup again: keep what is already saved.
    KeepMyData,
}

/// What the wizard hands back when the person presses Start.
pub(super) struct WizardResult {
    pub profile: Profile,
    pub start: StartWith,
}

pub(super) struct Wizard {
    step: Step,
    profile: Profile,
    start: StartWith,
    has_data: bool,
    error: String,
}

impl Wizard {
    pub fn new(existing: Option<Profile>, has_data: bool) -> Self {
        let start = if has_data { StartWith::KeepMyData } else { StartWith::SampleData };
        Self {
            step: Step::Welcome,
            profile: existing.unwrap_or_default(),
            start,
            has_data,
            error: String::new(),
        }
    }

    fn check_about_you(&self) -> Result<(), String> {
        let p = &self.profile;
        if p.first_name.trim().is_empty() {
            return Err("Please enter your first name.".into());
        }
        let email = p.email.trim();
        if !email.is_empty() && !(email.contains('@') && email.contains('.')) {
            return Err("That email address doesn't look right.".into());
        }
        Ok(())
    }

    /// Draws the wizard. Returns the result once the person finishes.
    pub fn show(&mut self, ctx: &egui::Context, db_note: &str) -> Option<WizardResult> {
        let mut done = None;
        egui::CentralPanel::default()
            .frame(Frame::none().fill(BG).inner_margin(Margin::same(24.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.08).max(12.0));
                        ui.horizontal(|ui| {
                            let w = 200.0;
                            ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                            logo(ui, 30.0);
                            ui.label(RichText::new("CoinCraft").size(26.0).strong());
                        });
                        ui.add_space(14.0);
                        self.progress_dots(ui);
                        ui.add_space(14.0);
                    });
                    let width = ui.available_width().min(600.0);
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            card(ui, |ui| match self.step {
                                Step::Welcome => self.welcome(ui, db_note),
                                Step::AboutYou => self.about_you(ui),
                                Step::Ready => done = self.ready(ui),
                            });
                        });
                    });
                });
            });
        done
    }

    fn progress_dots(&self, ui: &mut Ui) {
        let n = Step::ALL.len();
        let gap = 150.0;
        let (rect, _) = ui.allocate_exact_size(vec2(gap * (n - 1) as f32 + 120.0, 52.0), Sense::hover());
        let p = ui.painter();
        let x0 = rect.left() + 60.0;
        let y = rect.top() + 12.0;
        let cur = self.step.index();
        for i in 0..n {
            let x = x0 + gap * i as f32;
            if i + 1 < n {
                let col = if i < cur { ACCENT } else { BORDER };
                p.line_segment([pos2(x + 14.0, y), pos2(x + gap - 14.0, y)], Stroke::new(2.0_f32, col));
            }
            let (fill, txt) = if i < cur {
                (ACCENT, Color32::WHITE)
            } else if i == cur {
                (ACCENT, Color32::WHITE)
            } else {
                (CARD_HI, MUTED)
            };
            p.circle_filled(pos2(x, y), 12.0, fill);
            let mark = if i < cur { "✔".to_string() } else { (i + 1).to_string() };
            p.text(pos2(x, y), Align2::CENTER_CENTER, mark, FontId::proportional(12.0), txt);
            let label_col = if i == cur { TEXT } else { MUTED };
            p.text(
                pos2(x, y + 28.0),
                Align2::CENTER_CENTER,
                Step::ALL[i].title(),
                FontId::proportional(13.0),
                label_col,
            );
        }
    }

    fn welcome(&mut self, ui: &mut Ui, db_note: &str) {
        ui.label(RichText::new("Welcome to CoinCraft").size(24.0).strong());
        ui.add_space(4.0);
        muted(ui, "Let's get you set up. It takes about a minute.");
        ui.add_space(10.0);
        for (icon, text) in [
            ("🔁", "Find and cancel recurring charges"),
            ("🎯", "Budget by category and see where your money goes"),
            ("💰", "Save toward goals automatically"),
            ("📈", "Track your net worth and credit score"),
            ("🔒", "Your data stays on this computer, in a local database"),
        ] {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).color(ACCENT));
                ui.label(text);
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new(db_note).color(MUTED).size(12.0));
        ui.add_space(10.0);
        row_right(
            ui,
            |_| {},
            |ui| {
                if primary(ui, "Get started  ▶").clicked() {
                    self.step = Step::AboutYou;
                }
            },
        );
    }

    fn about_you(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("About you").size(24.0).strong());
        muted(ui, "Only your first name is needed. You can change all of this later in Settings.");
        ui.add_space(10.0);
        let p = &mut self.profile;
        egui::Grid::new("wizard_profile")
            .num_columns(2)
            .spacing([14.0, 10.0])
            .show(ui, |ui| {
                ui.label("First name *");
                ui.add(
                    egui::TextEdit::singleline(&mut p.first_name)
                        .hint_text(hint("e.g. Alex"))
                        .desired_width(300.0),
                );
                ui.end_row();
                ui.label("Last name");
                ui.add(egui::TextEdit::singleline(&mut p.last_name).desired_width(300.0));
                ui.end_row();
                ui.label("Email");
                ui.add(
                    egui::TextEdit::singleline(&mut p.email)
                        .hint_text(hint("you@example.com"))
                        .desired_width(300.0),
                );
                ui.end_row();
                ui.label("Phone");
                ui.add(egui::TextEdit::singleline(&mut p.phone).desired_width(300.0));
                ui.end_row();
                ui.label("Monthly take-home pay");
                money_field(ui, &mut p.monthly_income);
                ui.end_row();
            });
        if !self.error.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(self.error.as_str()).color(ACCENT));
        }
        ui.add_space(10.0);
        let mut back = false;
        let mut next = false;
        row_right(
            ui,
            |ui| back = ui.button("◀  Back").clicked(),
            |ui| next = primary(ui, "Continue  ▶").clicked(),
        );
        if back {
            self.error.clear();
            self.step = Step::Welcome;
        }
        if next {
            match self.check_about_you() {
                Ok(()) => {
                    self.error.clear();
                    self.step = Step::Ready;
                }
                Err(e) => self.error = e,
            }
        }
    }

    fn ready(&mut self, ui: &mut Ui) -> Option<WizardResult> {
        let p = &self.profile;
        ui.label(
            RichText::new(format!("You're all set, {}!", p.first_name.trim()))
                .size(24.0)
                .strong(),
        );
        muted(ui, "Here's what we'll save. Bank connections can be added later.");
        ui.add_space(8.0);
        egui::Grid::new("wizard_summary")
            .num_columns(2)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                let none = "—".to_string();
                let show = |s: &str| if s.trim().is_empty() { none.clone() } else { s.trim().to_string() };
                for (k, v) in [
                    ("Name", show(&p.full_name())),
                    ("Email", show(&p.email)),
                    ("Phone", show(&p.phone)),
                    (
                        "Monthly take-home pay",
                        if p.monthly_income > 0.0 {
                            money(p.monthly_income)
                        } else {
                            none.clone()
                        },
                    ),
                ] {
                    muted(ui, k);
                    ui.label(v);
                    ui.end_row();
                }
            });
        ui.add_space(10.0);
        section(ui, "How would you like to start?");
        if self.has_data {
            ui.radio_value(&mut self.start, StartWith::KeepMyData, "Keep my current data");
        }
        ui.radio_value(
            &mut self.start,
            StartWith::SampleData,
            "With sample data, to explore what CoinCraft can do",
        );
        ui.radio_value(
            &mut self.start,
            StartWith::CleanSlate,
            "With a clean slate, ready for my own accounts",
        );
        if self.has_data && self.start != StartWith::KeepMyData {
            ui.label(RichText::new("This replaces the data you have now.").color(AMBER));
        }
        ui.add_space(10.0);
        let mut back = false;
        let mut start = false;
        row_right(
            ui,
            |ui| back = ui.button("◀  Back").clicked(),
            |ui| start = primary(ui, "Start using CoinCraft").clicked(),
        );
        if back {
            self.step = Step::AboutYou;
        }
        if start {
            let mut profile = self.profile.clone();
            profile.first_name = profile.first_name.trim().to_string();
            profile.last_name = profile.last_name.trim().to_string();
            profile.email = profile.email.trim().to_string();
            profile.phone = profile.phone.trim().to_string();
            profile.setup_complete = true;
            return Some(WizardResult {
                profile,
                start: self.start,
            });
        }
        None
    }
}
