//! Dashboard, recurring charges, spending and budgets.

use super::widgets::*;
use super::*;
use chrono::Duration;

impl App {
    pub(super) fn page_dashboard(&mut self, ui: &mut Ui) {
        let s = &self.state;
        let t = today();
        let hour = chrono::Local::now().format("%H").to_string().parse::<u32>().unwrap_or(12);
        let greet = match hour {
            0..=11 => "Good morning",
            12..=17 => "Good afternoon",
            _ => "Good evening",
        };
        title(
            ui,
            &format!("{}, {}", greet, s.profile.first_name),
            &t.format("%A, %B %-d").to_string(),
        );

        let nw = s.net_worth();
        let prev_nw = if s.net_worth_history.len() >= 2 {
            s.net_worth_history[s.net_worth_history.len() - 2].1
        } else {
            nw
        };
        let spent = s.spent_in(t, None);
        let budget_total: f64 = s.budgets.iter().map(|b| b.limit).sum();
        let budget_spent: f64 = s.budgets.iter().map(|b| s.spent_in(t, Some(b.category))).sum();
        let subs = s.subs_monthly_total();
        let sub_count = s.active_subs().count();
        let score = s.credit.score();
        let score_change = score as i32 - s.credit.history.iter().rev().nth(1).map(|h| h.1 as i32).unwrap_or(score as i32);

        ui.columns(4, |c| {
            let ch = nw - prev_nw;
            stat(
                &mut c[0],
                "Net worth",
                &money0(nw),
                &format!("{} {} this month", if ch >= 0.0 { "⬆" } else { "⬇" }, money0(ch.abs())),
                if ch >= 0.0 { GREEN } else { ACCENT },
            );
            stat(
                &mut c[1],
                "Spent this month",
                &money0(spent),
                &format!("{} of {} budgeted", money0(budget_spent), money0(budget_total)),
                budget_color(budget_spent / budget_total.max(1.0)),
            );
            stat(
                &mut c[2],
                "Recurring each month",
                &money0(subs),
                &format!("{} active charges", sub_count),
                MUTED,
            );
            if s.credit.has_score() {
                let (band, col) = score_band(score);
                stat(
                    &mut c[3],
                    "Credit score",
                    &score.to_string(),
                    &format!("{} · {:+} pts", band, score_change),
                    col,
                );
            } else {
                stat(&mut c[3], "Credit score", "—", "Not connected yet", MUTED);
            }
        });
        ui.add_space(6.0);

        let flow: Vec<(String, f64, f64)> = (0..6)
            .rev()
            .map(|b| {
                let m = add_months(t, -b);
                (m.format("%b").to_string(), s.income_in(m), s.spent_in(m, None))
            })
            .collect();
        let alerts = s.compute_alerts();
        let mut upcoming: Vec<&Subscription> = s.active_subs().collect();
        upcoming.sort_by_key(|x| x.next_charge());

        let mut go = None;
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                row_right(
                    ui,
                    |ui| section(ui, "Cash flow"),
                    |ui| {
                        ui.label(RichText::new("■ Spending").color(ACCENT).size(12.0));
                        ui.label(RichText::new("■ Income").color(GREEN).size(12.0));
                    },
                );
                cash_flow_chart(ui, &flow, 236.0);
            });
            card(&mut c[1], |ui| {
                ui.horizontal(|ui| {
                    section(ui, "Alerts");
                    chip(ui, &alerts.len().to_string(), if alerts.is_empty() { GREEN } else { AMBER });
                });
                if alerts.is_empty() {
                    muted(ui, "All clear. Nothing needs your attention.");
                }
                for a in alerts.iter().take(5) {
                    alert_row(ui, a);
                }
                if alerts.len() > 5 && ui.link(format!("See all {} alerts", alerts.len())).clicked() {
                    go = Some(Page::Budgets);
                }
            });
        });
        ui.add_space(6.0);
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                row_right(
                    ui,
                    |ui| section(ui, "Upcoming charges"),
                    |ui| {
                        if ui.link("View all").clicked() {
                            go = Some(Page::Recurring);
                        }
                    },
                );
                for sub in upcoming.iter().take(5) {
                    row_right(
                        ui,
                        |ui| {
                            ui.label(RichText::new(sub.category.icon()).color(sub.category.color()));
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(sub.merchant.as_str()).strong());
                                let when = format!("{} · {}", sub.next_charge().format("%b %-d"), in_days(sub.next_charge()));
                                ui.label(RichText::new(when).color(MUTED).size(12.5));
                            });
                        },
                        |ui| {
                            ui.label(RichText::new(money(sub.amount)).strong());
                        },
                    );
                }
            });
            card(&mut c[1], |ui| {
                row_right(
                    ui,
                    |ui| section(ui, "Budgets this month"),
                    |ui| {
                        if ui.link("Manage").clicked() {
                            go = Some(Page::Budgets);
                        }
                    },
                );
                let mut list: Vec<(&Budget, f64)> = s.budgets.iter().map(|b| (b, s.spent_in(t, Some(b.category)))).collect();
                list.sort_by(|a, b| (b.1 / b.0.limit.max(1.0)).partial_cmp(&(a.1 / a.0.limit.max(1.0))).unwrap());
                for (b, spent) in list.iter().take(5) {
                    let f = spent / b.limit.max(1.0);
                    row_right(
                        ui,
                        |ui| {
                            ui.label(format!("{} {}", b.category.icon(), b.category.name()));
                        },
                        |ui| {
                            ui.label(RichText::new(format!("{} / {}", money0(*spent), money0(b.limit))).color(MUTED));
                        },
                    );
                    progress(ui, f, budget_color(f), 8.0);
                }
            });
        });
        if let Some(p) = go {
            self.page = p;
        }
    }

    pub(super) fn page_recurring(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Recurring charges",
            "Subscriptions and bills we found by scanning your accounts for repeat charges.",
        );
        let ctx = ui.ctx().clone();
        let monthly = self.state.subs_monthly_total();
        let cancelled: f64 = self
            .state
            .subscriptions
            .iter()
            .filter(|s| s.status == SubStatus::Cancelled)
            .map(|s| s.monthly_cost() * 12.0)
            .sum();
        let count = self.state.active_subs().count();
        let due_week: f64 = self
            .state
            .active_subs()
            .filter(|s| (s.next_charge() - today()).num_days() <= 7)
            .map(|s| s.amount)
            .sum();
        ui.columns(4, |c| {
            stat(&mut c[0], "Per month", &money(monthly), &format!("{} active", count), MUTED);
            stat(&mut c[1], "Per year", &money0(monthly * 12.0), "if nothing changes", MUTED);
            stat(&mut c[2], "Due in the next 7 days", &money(due_week), "across all accounts", AMBER);
            stat(&mut c[3], "Saved by cancelling", &money0(cancelled), "per year", GREEN);
        });
        ui.add_space(6.0);

        enum Act {
            Cancel(usize),
            Concierge(usize),
            Undo(usize),
            Remind(usize, bool),
        }
        let mut act = None;
        let mut rescan = false;
        let mut order: Vec<usize> = (0..self.state.subscriptions.len()).collect();
        order.sort_by_key(|&i| {
            let s = &self.state.subscriptions[i];
            (s.status == SubStatus::Cancelled, s.next_charge())
        });

        let state = &self.state;
        card(ui, |ui| {
            row_right(
                ui,
                |ui| section(ui, "Your recurring charges"),
                |ui| {
                    if primary(ui, "🔍 Scan accounts again").clicked() {
                        rescan = true;
                    }
                },
            );
            ui.add_space(4.0);
            egui::Grid::new("subs")
                .num_columns(7)
                .spacing([18.0, 10.0])
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for h in ["Name", "How often", "Amount", "Next charge", "Account", "Remind me", ""] {
                        ui.label(RichText::new(h).color(MUTED).size(12.5));
                    }
                    ui.end_row();
                    for &i in &order {
                        let s = &state.subscriptions[i];
                        let off = s.status == SubStatus::Cancelled;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(s.category.icon()).color(s.category.color()));
                            let name = RichText::new(s.merchant.as_str()).strong();
                            ui.label(if off { name.strikethrough().color(MUTED) } else { name });
                        });
                        muted(ui, s.cadence_name());
                        ui.label(RichText::new(money(s.amount)).strong());
                        if off {
                            muted(ui, "—");
                        } else {
                            ui.label(format!("{} · {}", s.next_charge().format("%b %-d"), in_days(s.next_charge())));
                        }
                        muted(ui, state.account_name(s.account));
                        let mut r = s.remind;
                        if ui.add_enabled(!off, egui::Checkbox::without_text(&mut r)).changed() {
                            act = Some(Act::Remind(i, r));
                        }
                        ui.horizontal(|ui| match s.status {
                            SubStatus::Active => {
                                if ui
                                    .button("Cancel")
                                    .on_hover_text("Cancel it yourself; we show you how and mark it done.")
                                    .clicked()
                                {
                                    act = Some(Act::Cancel(i));
                                }
                                if ui
                                    .button("Concierge cancel")
                                    .on_hover_text("Our team cancels it for you.")
                                    .clicked()
                                {
                                    act = Some(Act::Concierge(i));
                                }
                            }
                            SubStatus::Cancelling { ready_at } => {
                                ui.spinner();
                                muted(ui, format!("Concierge working… {}s", (ready_at - now_secs()).max(0)));
                            }
                            SubStatus::Cancelled => {
                                chip(ui, "Cancelled", GREEN);
                                if ui.small_button("Undo").clicked() {
                                    act = Some(Act::Undo(i));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            if state.subscriptions.is_empty() {
                muted(ui, "No repeat charges found yet. Add or import transactions, then scan again.");
            }
        });

        if rescan {
            let n = self.state.rescan();
            let msg = if n == 0 {
                "Scan finished. No new recurring charges.".to_string()
            } else {
                format!("Scan finished. Found {} new recurring charge{}.", n, if n == 1 { "" } else { "s" })
            };
            self.toast(&ctx, msg);
        }
        match act {
            Some(Act::Cancel(i)) => {
                let s = &mut self.state.subscriptions[i];
                s.status = SubStatus::Cancelled;
                let msg = format!(
                    "{} marked cancelled. You'll save {} a year.",
                    s.merchant,
                    money(s.monthly_cost() * 12.0)
                );
                self.toast(&ctx, msg);
            }
            Some(Act::Concierge(i)) => {
                self.state.subscriptions[i].status = SubStatus::Cancelling { ready_at: now_secs() + 8 };
                let msg = format!("Our concierge is cancelling {} for you.", self.state.subscriptions[i].merchant);
                self.toast(&ctx, msg);
            }
            Some(Act::Undo(i)) => self.state.subscriptions[i].status = SubStatus::Active,
            Some(Act::Remind(i, r)) => self.state.subscriptions[i].remind = r,
            None => {}
        }
    }

    pub(super) fn page_spending(&mut self, ui: &mut Ui) {
        title(ui, "Spending", "Where your money went, sorted into categories.");
        let month = add_months(month_start(today()), self.spend_offset);
        ui.horizontal(|ui| {
            if ui.button("◀").clicked() {
                self.spend_offset -= 1;
            }
            ui.label(RichText::new(month.format("%B %Y").to_string()).size(18.0).strong());
            if ui.add_enabled(self.spend_offset < 0, egui::Button::new("▶")).clicked() {
                self.spend_offset += 1;
            }
        });
        let s = &self.state;
        let cats = s.spending_by_category(month);
        let total: f64 = cats.iter().map(|c| c.1).sum();
        let income = s.income_in(month);
        let prev = s.spent_in(add_months(month, -1), None);
        ui.columns(3, |c| {
            stat(&mut c[0], "Income", &money(income), "money in", GREEN);
            let ch = total - prev;
            stat(
                &mut c[1],
                "Spending",
                &money(total),
                &format!("{} {} vs last month", if ch > 0.0 { "⬆" } else { "⬇" }, money0(ch.abs())),
                if ch > 0.0 { ACCENT } else { GREEN },
            );
            let net = income - total;
            stat(
                &mut c[2],
                "Left over",
                &money(net),
                if net >= 0.0 {
                    "nice, you're ahead"
                } else {
                    "spent more than you earned"
                },
                if net >= 0.0 { GREEN } else { ACCENT },
            );
        });
        ui.add_space(6.0);
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                section(ui, "By category");
                ui.horizontal(|ui| {
                    let slices: Vec<(Color32, f64)> = cats.iter().map(|(c, v)| (c.color(), *v)).collect();
                    donut(ui, &slices, 220.0, &money0(total), "spent");
                    ui.vertical(|ui| {
                        for (c, v) in &cats {
                            row_right(
                                ui,
                                |ui| {
                                    ui.label(RichText::new("■").color(c.color()));
                                    ui.label(c.name());
                                },
                                |ui| {
                                    ui.label(RichText::new(format!("{:.0}%", v / total.max(1.0) * 100.0)).color(MUTED));
                                    ui.label(RichText::new(money(*v)).strong());
                                },
                            );
                        }
                    });
                });
            });
            card(&mut c[1], |ui| {
                section(ui, "Top merchants");
                let mut m: Vec<(String, f64, Category)> = Vec::new();
                for t in s.in_month(month).filter(|t| t.amount < 0.0 && t.category.is_spending()) {
                    match m.iter_mut().find(|e| e.0 == t.merchant) {
                        Some(e) => e.1 += -t.amount,
                        None => m.push((t.merchant.clone(), -t.amount, t.category)),
                    }
                }
                m.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                for (name, v, cat) in m.iter().take(9) {
                    row_right(
                        ui,
                        |ui| {
                            ui.label(RichText::new(cat.icon()).color(cat.color()));
                            ui.label(name.as_str());
                        },
                        |ui| {
                            ui.label(RichText::new(money(*v)).strong());
                        },
                    );
                }
            });
        });
        ui.add_space(6.0);
        let flow: Vec<(String, f64, f64)> = (0..6)
            .rev()
            .map(|b| {
                let m = add_months(month, -b);
                (m.format("%b").to_string(), s.income_in(m), s.spent_in(m, None))
            })
            .collect();
        card(ui, |ui| {
            section(ui, "Cash flow, last 6 months");
            cash_flow_chart(ui, &flow, 240.0);
        });
    }

    pub(super) fn page_budgets(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Budgets",
            "Set a monthly limit for each category and we'll warn you before you go over.",
        );
        let t = today();
        let total_limit: f64 = self.state.budgets.iter().map(|b| b.limit).sum();
        let total_spent: f64 = self.state.budgets.iter().map(|b| self.state.spent_in(t, Some(b.category))).sum();
        let month_end = add_months(month_start(t), 1) - Duration::days(1);
        let days_left = (month_end - t).num_days() + 1;
        let left = total_limit - total_spent;
        ui.columns(3, |c| {
            stat(
                &mut c[0],
                "Budgeted this month",
                &money0(total_limit),
                &format!("{} spent", money0(total_spent)),
                MUTED,
            );
            stat(
                &mut c[1],
                "Left to spend",
                &money(left),
                &format!("{} days left in {}", days_left, t.format("%B")),
                budget_color(total_spent / total_limit.max(1.0)),
            );
            stat(
                &mut c[2],
                "Safe to spend per day",
                &money((left / days_left as f64).max(0.0)),
                "to stay on budget",
                GREEN,
            );
        });
        ui.add_space(6.0);

        let mut remove = None;
        let spent: Vec<f64> = self
            .state
            .budgets
            .iter()
            .map(|b| self.state.spent_in(t, Some(b.category)))
            .collect();
        let last: Vec<f64> = self
            .state
            .budgets
            .iter()
            .map(|b| self.state.spent_in(add_months(t, -1), Some(b.category)))
            .collect();
        let alerts = self.state.compute_alerts();
        let mut add = false;
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                section(ui, "Category budgets");
                for (i, b) in self.state.budgets.iter_mut().enumerate() {
                    let f = spent[i] / b.limit.max(1.0);
                    ui.add_space(4.0);
                    let cat = b.category;
                    row_right(
                        ui,
                        |ui| {
                            ui.label(RichText::new(cat.icon()).color(cat.color()));
                            ui.label(RichText::new(cat.name()).strong());
                        },
                        |ui| {
                            if ui.small_button("✖").on_hover_text("Remove budget").clicked() {
                                remove = Some(i);
                            }
                            money_field(ui, &mut b.limit);
                            muted(ui, "limit");
                        },
                    );
                    progress(ui, f, budget_color(f), 10.0);
                    let rem = b.limit - spent[i];
                    row_right(
                        ui,
                        |ui| {
                            ui.label(RichText::new(format!("{} spent", money(spent[i]))).size(12.5));
                        },
                        |ui| {
                            let txt = if rem >= 0.0 {
                                format!("{} left · last month {}", money(rem), money0(last[i]))
                            } else {
                                format!("{} over", money(-rem))
                            };
                            ui.label(RichText::new(txt).color(if rem >= 0.0 { MUTED } else { ACCENT }).size(12.5));
                        },
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    muted(ui, "Add a budget:");
                    category_combo(ui, "new_budget_cat", &mut self.new_budget_cat, 170.0);
                    money_field(ui, &mut self.new_budget_limit);
                    if primary(ui, "Add").clicked() {
                        add = true;
                    }
                });
            });
            let ui = &mut c[1];
            card(ui, |ui| {
                section(ui, "Alerts");
                if alerts.is_empty() {
                    muted(ui, "All clear right now.");
                }
                for a in &alerts {
                    alert_row(ui, a);
                }
            });
            ui.add_space(6.0);
            card(ui, |ui| {
                section(ui, "Alert settings");
                let a = &mut self.state.alerts;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut a.low_balance, "Low balance alert when checking drops under");
                    money_field(ui, &mut a.low_balance_threshold);
                });
                ui.checkbox(
                    &mut a.overdraft,
                    "Overdraft alerts (and warn me when upcoming bills could overdraw me)",
                );
                ui.checkbox(&mut a.budget, "Warn me at 85% and 100% of a budget");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut a.renewals, "Remind me before subscriptions renew, days ahead:");
                    ui.add(egui::DragValue::new(&mut a.renewal_days).speed(0.1));
                    a.renewal_days = a.renewal_days.clamp(0, 30);
                });
            });
        });
        if add {
            let (cat, limit) = (self.new_budget_cat, self.new_budget_limit);
            if let Some(b) = self.state.budgets.iter_mut().find(|b| b.category == cat) {
                b.limit = limit;
            } else {
                self.state.budgets.push(Budget { category: cat, limit });
            }
        }
        if let Some(i) = remove {
            self.state.budgets.remove(i);
        }
    }
}
