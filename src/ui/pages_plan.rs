//! Credit score, bill negotiation, the assistant and settings.

use super::widgets::*;
use super::*;
use crate::assistant;

impl App {
    pub(super) fn page_credit(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Credit score",
            "Track your score and what moves it. Checking here never hurts your score.",
        );
        let ctx = ui.ctx().clone();
        if !self.state.credit.has_score() {
            card(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("💳").size(40.0));
                    ui.label(RichText::new("No credit score yet").size(20.0).strong());
                    muted(
                        ui,
                        "Credit monitoring will connect here later. Your score and report will show up on this page.",
                    );
                    ui.add_space(20.0);
                });
            });
            return;
        }
        let score = self.state.credit.score();
        let util = self.state.card_utilization();
        let total_limit: f64 = self
            .state
            .accounts
            .iter()
            .filter(|a| a.kind == AccountKind::CreditCard)
            .map(|a| a.limit)
            .sum();
        let prev = self.state.credit.history.iter().rev().nth(1).map(|h| h.1).unwrap_or(score);
        let mut refresh = false;
        let cr = &self.state.credit;
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                ui.vertical_centered(|ui| {
                    gauge(ui, score, 360.0);
                    let ch = score as i32 - prev as i32;
                    ui.label(RichText::new(format!("{:+} points since last month", ch)).color(if ch >= 0 { GREEN } else { ACCENT }));
                    muted(ui, format!("Updated {}", today().format("%B %-d, %Y")));
                    if ui.button("⟳ Refresh score").clicked() {
                        refresh = true;
                    }
                });
            });
            card(&mut c[1], |ui| {
                section(ui, "Your credit report");
                let util_rating = if util < 0.1 {
                    ("Excellent", GREEN)
                } else if util < 0.3 {
                    ("Good", AMBER)
                } else {
                    ("Needs work", ACCENT)
                };
                let rows: [(&str, String, (&str, Color32), &str); 5] = [
                    (
                        "Payment history",
                        format!("{:.0}% on time", cr.on_time_pct),
                        if cr.on_time_pct >= 99.0 {
                            ("Excellent", GREEN)
                        } else {
                            ("Fair", AMBER)
                        },
                        "High impact",
                    ),
                    ("Card utilization", format!("{:.0}% used", util * 100.0), util_rating, "High impact"),
                    (
                        "Credit age",
                        format!("{:.1} years", cr.age_years),
                        if cr.age_years >= 7.0 {
                            ("Excellent", GREEN)
                        } else {
                            ("Good", AMBER)
                        },
                        "Medium impact",
                    ),
                    (
                        "Hard inquiries",
                        format!("{}", cr.hard_inquiries),
                        if cr.hard_inquiries <= 1 {
                            ("Excellent", GREEN)
                        } else {
                            ("Fair", AMBER)
                        },
                        "Low impact",
                    ),
                    (
                        "Derogatory marks",
                        format!("{}", cr.derogatory),
                        if cr.derogatory == 0 {
                            ("Excellent", GREEN)
                        } else {
                            ("Poor", ACCENT)
                        },
                        "High impact",
                    ),
                ];
                for (name, value, (rating, col), impact) in rows {
                    row_right(
                        ui,
                        |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(name).strong());
                                ui.label(RichText::new(impact).color(MUTED).size(12.0));
                            });
                        },
                        |ui| {
                            chip(ui, rating, col);
                            ui.label(value);
                        },
                    );
                    ui.separator();
                }
                let tip = if util >= 0.3 {
                    format!(
                        "Tip: paying your cards down by {} would bring utilization under 30%.",
                        money0((util - 0.29) * total_limit)
                    )
                } else {
                    "Tip: you're doing well. Keep balances low and pay on time every month.".to_string()
                };
                ui.label(RichText::new(tip).color(BLUE));
            });
        });
        ui.add_space(6.0);
        let skip = cr.history.len().saturating_sub(12);
        let hist: Vec<(String, f64)> = cr
            .history
            .iter()
            .skip(skip)
            .map(|(d, v)| (d.format("%b").to_string(), *v as f64))
            .collect();
        card(ui, |ui| {
            section(ui, &format!("Score history, last {} months", hist.len()));
            line_chart(ui, &hist, 210.0, GREEN, |v| format!("{:.0}", v));
        });
        if refresh {
            let delta: i32 = if util < 0.3 { 2 } else { -3 };
            let m = month_start(today());
            let new = (score as i32 + delta).clamp(300, 850) as u16;
            let cr = &mut self.state.credit;
            match cr.history.last_mut() {
                Some(last) if last.0 == m => last.1 = new,
                _ => cr.history.push((m, new)),
            }
            self.toast(&ctx, format!("Score refreshed: {} ({:+}).", new, delta));
        }
    }

    pub(super) fn page_bills(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Bill negotiation",
            "Our team asks your providers for a lower rate. You only pay if we save you money.",
        );
        let ctx = ui.ctx().clone();
        let yearly: f64 = self
            .state
            .bills
            .iter()
            .filter_map(|b| match b.status {
                NegStatus::Saved { new_monthly, .. } => Some((b.monthly - new_monthly) * 12.0),
                _ => None,
            })
            .sum();
        let fees: f64 = self
            .state
            .bills
            .iter()
            .filter_map(|b| match b.status {
                NegStatus::Saved { fee, .. } => Some(fee),
                _ => None,
            })
            .sum();
        ui.columns(3, |c| {
            stat(&mut c[0], "Saved per year", &money(yearly), "from negotiated bills", GREEN);
            stat(&mut c[1], "Success fees", &money(fees), "charged once, only when it works", MUTED);
            stat(
                &mut c[2],
                "You keep",
                &money(yearly - fees),
                "in the first year, then all of it",
                BLUE,
            );
        });
        ui.add_space(6.0);

        let mut start = None;
        let mut retry = None;
        let mut remove = None;
        let fee_pct = self.state.fee_pct;
        card(ui, |ui| {
            row_right(
                ui,
                |ui| section(ui, "Your bills"),
                |ui| {
                    ui.add(egui::Slider::new(&mut self.state.fee_pct, 35.0..=60.0).suffix("%").step_by(5.0));
                    muted(ui, "Choose your fee (share of first-year savings):");
                },
            );
            egui::Grid::new("bills")
                .num_columns(5)
                .spacing([22.0, 12.0])
                .striped(true)
                .min_col_width(70.0)
                .show(ui, |ui| {
                    for h in ["Provider", "Type", "Current", "Status", ""] {
                        ui.label(RichText::new(h).color(MUTED).size(12.5));
                    }
                    ui.end_row();
                    for (i, b) in self.state.bills.iter().enumerate() {
                        ui.label(RichText::new(b.provider.as_str()).strong());
                        ui.horizontal(|ui| {
                            ui.label(b.kind.name());
                            if !b.kind.eligible() {
                                chip(ui, "Not eligible", MUTED);
                            }
                        });
                        ui.label(format!("{}/mo", money(b.monthly)));
                        match b.status {
                            NegStatus::NotStarted => {
                                muted(
                                    ui,
                                    if b.kind.eligible() {
                                        "Ready to negotiate"
                                    } else {
                                        "We can't negotiate this type yet"
                                    },
                                );
                                if b.kind.eligible() {
                                    if primary(ui, "Negotiate").clicked() {
                                        start = Some(i);
                                    }
                                } else {
                                    ui.label("");
                                }
                            }
                            NegStatus::Negotiating { ready_at } => {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(format!("Talking to the provider… ({}s)", (ready_at - now_secs()).max(0)));
                                });
                                ui.label("");
                            }
                            NegStatus::Saved { new_monthly, fee } => {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(RichText::new(format!("Lowered to {}/mo", money(new_monthly))).color(GREEN).strong());
                                    let detail = format!("{} a year saved · fee {}", money((b.monthly - new_monthly) * 12.0), money(fee));
                                    ui.label(RichText::new(detail).color(MUTED).size(12.5));
                                });
                                chip(ui, "Success", GREEN);
                            }
                            NegStatus::NoSavings => {
                                muted(ui, "No better rate this time. No fee.");
                                if ui.button("Try again").clicked() {
                                    retry = Some(i);
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
        });
        ui.add_space(6.0);
        let mut add = false;
        card(ui, |ui| {
            section(ui, "Add a bill");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.bill_provider)
                        .hint_text(hint("Provider name"))
                        .desired_width(200.0),
                );
                egui::ComboBox::new("bill_kind", "")
                    .selected_text(self.bill_kind.name())
                    .show_ui(ui, |ui| {
                        for k in BillKind::ALL {
                            ui.selectable_value(&mut self.bill_kind, k, k.name());
                        }
                    });
                money_field(ui, &mut self.bill_monthly);
                muted(ui, "per month");
                if primary(ui, "Add bill").clicked() && !self.bill_provider.trim().is_empty() {
                    add = true;
                }
                if !self.state.bills.is_empty() && ui.button("Remove last").clicked() {
                    remove = Some(self.state.bills.len() - 1);
                }
            });
            muted(
                ui,
                format!(
                    "How it works: we contact the provider for you and ask for a better rate on internet, phone, cable and insurance bills. If it works you pay {:.0}% of the first year's savings, once. If not, you pay nothing. (In this offline version the negotiation is simulated.)",
                    fee_pct
                ),
            );
        });

        if add {
            let id = self.state.bills.iter().map(|b| b.id).max().unwrap_or(0) + 1;
            self.state.bills.push(Bill {
                id,
                provider: self.bill_provider.trim().to_string(),
                kind: self.bill_kind,
                monthly: self.bill_monthly,
                status: NegStatus::NotStarted,
            });
            self.bill_provider.clear();
        }
        if let Some(i) = start {
            self.state.bills[i].status = NegStatus::Negotiating { ready_at: now_secs() + 8 };
            let msg = format!("Negotiation with {} has started.", self.state.bills[i].provider);
            self.toast(&ctx, msg);
        }
        if let Some(i) = retry {
            self.state.bills[i].monthly = (self.state.bills[i].monthly * 100.0 + 1.0).round() / 100.0;
            self.state.bills[i].status = NegStatus::NotStarted;
        }
        if let Some(i) = remove {
            self.state.bills.remove(i);
        }
    }

    pub(super) fn page_assistant(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Money assistant",
            "Ask about your spending, budgets, subscriptions and goals. Answers come from your own data, on this computer.",
        );
        if self.state.chat.is_empty() {
            self.state.chat.push(ChatMsg {
                from_user: false,
                text: format!(
                    "Hi {}! I can look through your accounts and answer questions. What would you like to know?",
                    self.state.profile.first_name
                ),
            });
        }
        let mut ask: Option<String> = None;
        let h = (ui.ctx().screen_rect().height() - 360.0).max(240.0);
        card(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(h)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let bubble_width = ui.available_width() * 0.62;
                    for m in &self.state.chat {
                        // Your messages sit on the right, answers on the left; long text wraps.
                        let align = if m.from_user { Align::Max } else { Align::Min };
                        ui.with_layout(Layout::top_down(align), |ui| {
                            Frame::none()
                                .fill(if m.from_user { Color32::from_rgb(120, 48, 56) } else { CARD_HI })
                                .rounding(12.0)
                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                .show(ui, |ui| {
                                    ui.set_max_width(bubble_width);
                                    ui.add(egui::Label::new(m.text.as_str()).wrap());
                                });
                        });
                        ui.add_space(4.0);
                    }
                });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for s in assistant::SUGGESTIONS {
                    if ui.button(s).clicked() {
                        ask = Some(s.to_string());
                    }
                }
            });
            ui.horizontal(|ui| {
                let w = ui.available_width() - 90.0;
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.chat_input)
                        .hint_text(hint("Ask anything about your money…"))
                        .desired_width(w),
                );
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (primary(ui, "Send").clicked() || enter) && !self.chat_input.trim().is_empty() {
                    ask = Some(std::mem::take(&mut self.chat_input));
                    resp.request_focus();
                }
            });
        });
        if let Some(q) = ask {
            let reply = assistant::answer(&self.state, &q);
            self.state.chat.push(ChatMsg {
                from_user: true,
                text: q.trim().to_string(),
            });
            self.state.chat.push(ChatMsg {
                from_user: false,
                text: reply,
            });
        }
    }

    pub(super) fn page_settings(&mut self, ui: &mut Ui) {
        title(ui, "Settings", "");
        let ctx = ui.ctx().clone();
        card(ui, |ui| {
            section(ui, "Profile");
            let p = &mut self.state.profile;
            egui::Grid::new("settings_profile")
                .num_columns(2)
                .spacing([14.0, 8.0])
                .show(ui, |ui| {
                    ui.label("First name");
                    ui.add(egui::TextEdit::singleline(&mut p.first_name).desired_width(260.0));
                    ui.end_row();
                    ui.label("Last name");
                    ui.add(egui::TextEdit::singleline(&mut p.last_name).desired_width(260.0));
                    ui.end_row();
                    ui.label("Email");
                    ui.add(egui::TextEdit::singleline(&mut p.email).desired_width(260.0));
                    ui.end_row();
                    ui.label("Phone");
                    ui.add(egui::TextEdit::singleline(&mut p.phone).desired_width(260.0));
                    ui.end_row();
                    ui.label("Monthly take-home pay");
                    money_field(ui, &mut p.monthly_income);
                    ui.end_row();
                });
        });
        ui.add_space(6.0);

        let mut save_now = false;
        let mut rerun = false;
        card(ui, |ui| {
            section(ui, "Database");
            muted(ui, format!("CoinCraft version {}", env!("CARGO_PKG_VERSION")));
            muted(
                ui,
                "CoinCraft keeps everything in a local SQLite database file and saves changes automatically every few seconds. Only bank sync goes online, and only when you connect or press Sync. Saved logins are encrypted with your master password (see Connections).",
            );
            ui.label(RichText::new(self.db_status.as_str()).color(if self.db.is_some() { BLUE } else { AMBER }));
            ui.horizontal(|ui| {
                if ui.add_enabled(self.db.is_some(), egui::Button::new("Save now")).clicked() {
                    save_now = true;
                }
                if ui
                    .button("Run setup again")
                    .on_hover_text("Opens the welcome wizard. You can keep your data.")
                    .clicked()
                {
                    rerun = true;
                }
            });
        });
        ui.add_space(6.0);

        let mut reset = None;
        card(ui, |ui| {
            section(ui, "Start over");
            muted(
                ui,
                "Replace everything with sample data to explore, or clear it all for a clean slate. Your profile is kept.",
            );
            ui.horizontal(|ui| match self.confirm_reset {
                None => {
                    if ui.button("Load sample data").clicked() {
                        self.confirm_reset = Some(true);
                    }
                    if ui.button("Clear all data").clicked() {
                        self.confirm_reset = Some(false);
                    }
                }
                Some(with_sample) => {
                    ui.label(RichText::new("This replaces all your data. Are you sure?").color(AMBER));
                    if primary(
                        ui,
                        if with_sample {
                            "Yes, load sample data"
                        } else {
                            "Yes, clear everything"
                        },
                    )
                    .clicked()
                    {
                        reset = Some(with_sample);
                    }
                    if ui.button("Keep my data").clicked() {
                        self.confirm_reset = None;
                    }
                }
            });
        });

        if save_now {
            self.last_saved.clear();
            self.persist();
            self.toast(&ctx, "Saved.");
        }
        if rerun {
            self.rerun_setup();
        }
        if let Some(with_sample) = reset {
            let profile = self.state.profile.clone();
            self.confirm_reset = None;
            self.start_fresh(profile, with_sample);
            self.page = Page::Settings;
            self.toast(&ctx, if with_sample { "Sample data loaded." } else { "All data cleared." });
        }
    }
}
