//! Transactions (with import, rules and categories), net worth and savings goals.

use super::widgets::*;
use super::*;
use chrono::NaiveDate;

impl App {
    pub(super) fn page_transactions(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Transactions",
            "Every charge and deposit, with tidy merchant names and categories picked automatically. Change any category with a click.",
        );
        let ctx = ui.ctx().clone();
        let accounts: Vec<(u32, String)> = self.state.accounts.iter().map(|a| (a.id, a.name.clone())).collect();
        let acct_name = |id: u32| accounts.iter().find(|a| a.0 == id).map(|a| a.1.clone()).unwrap_or_default();

        let mut add_tx = false;
        let mut import = false;
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                section(ui, "Add a transaction");
                egui::Grid::new("newtx").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Date");
                    ui.add(egui::TextEdit::singleline(&mut self.new_tx.date).hint_text(hint("YYYY-MM-DD")).desired_width(140.0));
                    ui.end_row();
                    ui.label("Description");
                    ui.add(egui::TextEdit::singleline(&mut self.new_tx.merchant).hint_text(hint("e.g. SQ *CORNER MARKET #12")).desired_width(240.0));
                    ui.end_row();
                    ui.label("Amount");
                    ui.horizontal(|ui| {
                        money_field(ui, &mut self.new_tx.amount);
                        ui.selectable_value(&mut self.new_tx.income, false, "Money out");
                        ui.selectable_value(&mut self.new_tx.income, true, "Money in");
                    });
                    ui.end_row();
                    ui.label("Category");
                    egui::ComboBox::new("newtx_cat", "")
                        .selected_text(self.new_tx.category.map(|c| c.name()).unwrap_or("Pick for me"))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.new_tx.category, None, "Pick for me");
                            for c in Category::all() {
                                ui.selectable_value(&mut self.new_tx.category, Some(c), c.name());
                            }
                        });
                    ui.end_row();
                    ui.label("Account");
                    egui::ComboBox::new("newtx_acct", "").selected_text(acct_name(self.new_tx.account)).show_ui(ui, |ui| {
                        for (id, name) in &accounts {
                            ui.selectable_value(&mut self.new_tx.account, *id, name.as_str());
                        }
                    });
                    ui.end_row();
                });
                if primary(ui, "Add transaction").clicked() {
                    add_tx = true;
                }
            });
            card(&mut c[1], |ui| {
                section(ui, "Import a bank or card statement");
                muted(ui, "Download a statement from your bank or card website. OFX and QFX files bring in the account itself and skip anything already imported. CSV files need columns: date, description, amount (and optionally category), with spending negative.");
                ui.horizontal(|ui| {
                    ui.label("File");
                    let w = ui.available_width() - 10.0;
                    ui.add(egui::TextEdit::singleline(&mut self.csv_path).hint_text(hint("/path/to/statement.ofx or .csv")).desired_width(w));
                });
                ui.horizontal(|ui| {
                    ui.label("Into account");
                    egui::ComboBox::new("csv_acct", "").selected_text(acct_name(self.csv_account)).show_ui(ui, |ui| {
                        for (id, name) in &accounts {
                            ui.selectable_value(&mut self.csv_account, *id, name.as_str());
                        }
                    });
                });
                muted(ui, "Tip: you can also drop the file onto the window. \"Into account\" is only used for CSV files.");
                if primary(ui, "Import").clicked() {
                    import = true;
                }
                if !self.csv_msg.is_empty() {
                    ui.label(RichText::new(self.csv_msg.as_str()).color(AMBER));
                }
            });
        });
        let dropped: Option<PathBuf> = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));
        if let Some(p) = dropped {
            self.csv_path = p.display().to_string();
        }
        if add_tx {
            match NaiveDate::parse_from_str(self.new_tx.date.trim(), "%Y-%m-%d") {
                Err(_) => self.toast(&ctx, "Please write the date like 2026-11-19."),
                Ok(_) if self.new_tx.merchant.trim().is_empty() || self.new_tx.amount <= 0.0 => {
                    self.toast(&ctx, "Add a description and an amount first.")
                }
                Ok(date) => {
                    let amount = if self.new_tx.income {
                        self.new_tx.amount
                    } else {
                        -self.new_tx.amount
                    };
                    let (merchant, guessed) = self.state.categorize(&self.new_tx.merchant, amount);
                    let cat = self.new_tx.category.unwrap_or(guessed);
                    self.state.add_transaction(date, merchant.clone(), amount, cat, self.new_tx.account);
                    self.state.transactions.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
                    self.state.refresh_net_worth();
                    self.new_tx.merchant.clear();
                    self.new_tx.amount = 0.0;
                    self.toast(&ctx, format!("Added {} as {}.", merchant, cat.name()));
                }
            }
        }
        if import {
            let path = self.csv_path.trim().trim_matches('"').to_string();
            let is_ofx = {
                let lower = path.to_lowercase();
                lower.ends_with(".ofx") || lower.ends_with(".qfx")
            };
            match std::fs::read_to_string(&path) {
                Err(e) => self.csv_msg = format!("Couldn't open that file: {e}"),
                // OFX/QFX statements carry their own account, so they don't need "Into account".
                Ok(text) if is_ofx || text.to_ascii_uppercase().contains("<OFX>") => match parse_ofx(&text) {
                    Err(e) => self.csv_msg = e,
                    Ok(accounts) => {
                        let n = accounts.len();
                        let r = self.state.merge_remote(accounts);
                        self.csv_msg = format!(
                            "Read {} account{} from the statement: {} new transaction{}{}{}.",
                            n,
                            if n == 1 { "" } else { "s" },
                            r.new_transactions,
                            if r.new_transactions == 1 { "" } else { "s" },
                            if r.new_accounts > 0 {
                                format!(
                                    ", {} new account{} added",
                                    r.new_accounts,
                                    if r.new_accounts == 1 { "" } else { "s" }
                                )
                            } else {
                                String::new()
                            },
                            if r.skipped_duplicates > 0 {
                                format!(", {} already here and skipped", r.skipped_duplicates)
                            } else {
                                String::new()
                            },
                        );
                    }
                },
                Ok(text) => {
                    let (rows, skipped) = parse_csv(&text);
                    let n = rows.len();
                    for (date, desc, amount, cat) in rows {
                        let (merchant, guessed) = self.state.categorize(&desc, amount);
                        self.state
                            .add_transaction(date, merchant, amount, cat.unwrap_or(guessed), self.csv_account);
                    }
                    self.state.transactions.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
                    let found = self.state.rescan();
                    self.state.refresh_net_worth();
                    let skip = if skipped > 0 {
                        format!(", skipped {} line{} we couldn't read", skipped, if skipped == 1 { "" } else { "s" })
                    } else {
                        String::new()
                    };
                    self.csv_msg = format!(
                        "Imported {} transaction{}{}. Found {} new recurring charge{}.",
                        n,
                        if n == 1 { "" } else { "s" },
                        skip,
                        found,
                        if found == 1 { "" } else { "s" }
                    );
                }
            }
        }
        ui.add_space(6.0);

        // Rules and custom categories.
        let mut add_rule = false;
        let mut remove_rule = None;
        let mut run_rules = false;
        let mut add_cat = false;
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                section(ui, "Transaction rules");
                muted(
                    ui,
                    "Automatically rename and categorize matching transactions, now and when new ones arrive.",
                );
                for (i, r) in self.state.rules.iter().enumerate() {
                    row_right(
                        ui,
                        |ui| {
                            let rename = if r.rename.trim().is_empty() {
                                String::new()
                            } else {
                                format!(", rename to \"{}\"", r.rename.trim())
                            };
                            ui.label(format!("If it contains \"{}\": {}{}", r.contains.trim(), r.category.name(), rename));
                        },
                        |ui| {
                            if ui.small_button("✖").clicked() {
                                remove_rule = Some(i);
                            }
                        },
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.rule_contains)
                            .hint_text(hint("Contains…"))
                            .desired_width(120.0),
                    );
                    category_combo(ui, "rule_cat", &mut self.rule_cat, 160.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.rule_rename)
                            .hint_text(hint("Rename to (optional)"))
                            .desired_width(150.0),
                    );
                    if primary(ui, "Add rule").clicked() && !self.rule_contains.trim().is_empty() {
                        add_rule = true;
                    }
                    if !self.state.rules.is_empty() && ui.button("Apply to past transactions").clicked() {
                        run_rules = true;
                    }
                });
            });
            card(&mut c[1], |ui| {
                section(ui, "Your own categories");
                let n = self.state.custom_categories.len();
                if n == 0 {
                    muted(ui, "Make categories that fit your life, like \"Pets\" or \"Kids\".");
                } else {
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in self.state.custom_categories.iter().enumerate() {
                            chip(ui, &format!("🏷 {}", name), Category::Custom(i as u16).color());
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_custom_cat)
                            .hint_text(hint("Category name"))
                            .desired_width(180.0),
                    );
                    if primary(ui, "Add category").clicked() && !self.new_custom_cat.trim().is_empty() {
                        add_cat = true;
                    }
                });
            });
        });
        if add_rule {
            self.state.rules.push(Rule {
                contains: self.rule_contains.trim().to_string(),
                category: self.rule_cat,
                rename: self.rule_rename.trim().to_string(),
            });
            self.rule_contains.clear();
            self.rule_rename.clear();
            let n = self.state.apply_rules();
            self.toast(
                &ctx,
                format!("Rule added. {} past transaction{} updated.", n, if n == 1 { "" } else { "s" }),
            );
        }
        if run_rules {
            let n = self.state.apply_rules();
            self.toast(&ctx, format!("{} transaction{} updated.", n, if n == 1 { "" } else { "s" }));
        }
        if let Some(i) = remove_rule {
            self.state.rules.remove(i);
        }
        if add_cat {
            self.state.custom_categories.push(self.new_custom_cat.trim().to_string());
            set_custom_categories(&self.state.custom_categories);
            self.new_custom_cat.clear();
        }
        ui.add_space(6.0);

        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(
                    egui::TextEdit::singleline(&mut self.tx_search)
                        .hint_text(hint("Search"))
                        .desired_width(220.0),
                );
                egui::ComboBox::new("tx_cat", "")
                    .selected_text(self.tx_cat.map(|c| c.name()).unwrap_or("All categories"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.tx_cat, None, "All categories");
                        for c in Category::all() {
                            ui.selectable_value(&mut self.tx_cat, Some(c), c.name());
                        }
                    });
                let acct_text = match self.tx_account {
                    Some(id) => acct_name(id),
                    None => "All accounts".to_string(),
                };
                egui::ComboBox::new("tx_acct", "").selected_text(acct_text).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.tx_account, None, "All accounts");
                    for (id, name) in &accounts {
                        ui.selectable_value(&mut self.tx_account, Some(*id), name.as_str());
                    }
                });
            });
            let q = self.tx_search.to_lowercase();
            let (cat_f, acct_f) = (self.tx_cat, self.tx_account);
            let shown: Vec<usize> = self
                .state
                .transactions
                .iter()
                .enumerate()
                .filter(|(_, t)| q.is_empty() || t.merchant.to_lowercase().contains(&q))
                .filter(|(_, t)| cat_f.map_or(true, |c| t.category == c))
                .filter(|(_, t)| acct_f.map_or(true, |a| t.account == a))
                .map(|(i, _)| i)
                .collect();
            muted(ui, format!("{} transactions", shown.len()));
            egui::Grid::new("txs")
                .num_columns(5)
                .spacing([20.0, 6.0])
                .striped(true)
                .min_col_width(80.0)
                .show(ui, |ui| {
                    for h in ["Date", "Merchant", "Category", "Account", "Amount"] {
                        ui.label(RichText::new(h).color(MUTED).size(12.5));
                    }
                    ui.end_row();
                    for &i in shown.iter().take(250) {
                        let t = &mut self.state.transactions[i];
                        muted(ui, t.date.format("%b %-d, %Y").to_string());
                        ui.label(t.merchant.as_str());
                        category_combo(ui, &format!("cat{}", t.id), &mut t.category, 180.0);
                        muted(ui, acct_name(t.account));
                        amount_label(ui, t.amount);
                        ui.end_row();
                    }
                });
            if shown.len() > 250 {
                muted(ui, "Showing the newest 250. Search or filter to see older ones.");
            }
        });
    }

    pub(super) fn page_net_worth(&mut self, ui: &mut Ui) {
        title(ui, "Net worth", "Everything you own minus everything you owe, in one place.");
        let s = &self.state;
        let nw = s.net_worth();
        let first = s.net_worth_history.first().map(|h| h.1).unwrap_or(nw);
        let goals_total: f64 = s.goals.iter().map(|g| g.saved).sum();
        let debt_count = s.accounts.iter().filter(|a| a.kind.is_debt()).count();
        ui.columns(3, |c| {
            let ch = nw - first;
            stat(
                &mut c[0],
                "Net worth",
                &money0(nw),
                &format!(
                    "{} {} in {} months",
                    if ch >= 0.0 { "⬆" } else { "⬇" },
                    money0(ch.abs()),
                    s.net_worth_history.len()
                ),
                if ch >= 0.0 { GREEN } else { ACCENT },
            );
            stat(
                &mut c[1],
                "Assets",
                &money0(s.assets()),
                &format!("incl. {} in savings goals", money0(goals_total)),
                GREEN,
            );
            stat(&mut c[2], "Debts", &money0(s.debts()), &format!("{} accounts", debt_count), ACCENT);
        });
        ui.add_space(6.0);
        let skip = s.net_worth_history.len().saturating_sub(12);
        let hist: Vec<(String, f64)> = s
            .net_worth_history
            .iter()
            .skip(skip)
            .map(|(d, v)| (d.format("%b").to_string(), *v))
            .collect();
        card(ui, |ui| {
            if hist.len() < 2 {
                section(ui, "Net worth over time");
                muted(ui, "Your history builds up month by month from here.");
            } else {
                section(ui, &format!("Net worth over the last {} months", hist.len()));
            }
            line_chart(ui, &hist, 230.0, BLUE, money0);
        });
        ui.add_space(6.0);

        let mut changed = false;
        let mut remove = None;
        ui.columns(2, |c| {
            for (col, debt) in [(0usize, false), (1usize, true)] {
                card(&mut c[col], |ui| {
                    section(ui, if debt { "Debts" } else { "Assets" });
                    for (i, a) in self.state.accounts.iter_mut().enumerate().filter(|(_, a)| a.kind.is_debt() == debt) {
                        let name = a.name.clone();
                        let extra = if a.kind == AccountKind::CreditCard {
                            format!(" · {:.0}% of {} limit", a.balance / a.limit.max(1.0) * 100.0, money0(a.limit))
                        } else {
                            String::new()
                        };
                        let sub = format!("{}{}", a.kind.name(), extra);
                        row_right(
                            ui,
                            |ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(RichText::new(name).strong());
                                    ui.label(RichText::new(sub).color(MUTED).size(12.5));
                                });
                            },
                            |ui| {
                                if ui.small_button("✖").on_hover_text("Remove account").clicked() {
                                    remove = Some(i);
                                }
                                let before = a.balance;
                                money_field(ui, &mut a.balance);
                                if a.balance != before {
                                    changed = true;
                                }
                            },
                        );
                    }
                });
            }
        });
        ui.add_space(6.0);
        let mut add = false;
        card(ui, |ui| {
            section(ui, "Add an account");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.acct_name)
                        .hint_text(hint("Name, e.g. Home"))
                        .desired_width(200.0),
                );
                egui::ComboBox::new("acct_kind", "")
                    .selected_text(self.acct_kind.name())
                    .show_ui(ui, |ui| {
                        for k in AccountKind::ALL {
                            ui.selectable_value(&mut self.acct_kind, k, k.name());
                        }
                    });
                muted(ui, if self.acct_kind.is_debt() { "Owed" } else { "Value" });
                money_field(ui, &mut self.acct_balance);
                if self.acct_kind == AccountKind::CreditCard {
                    muted(ui, "Limit");
                    money_field(ui, &mut self.acct_limit);
                }
                if primary(ui, "Add account").clicked() && !self.acct_name.trim().is_empty() {
                    add = true;
                }
            });
        });
        if add {
            let id = self.state.accounts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
            self.state.accounts.push(Account {
                id,
                name: self.acct_name.trim().to_string(),
                kind: self.acct_kind,
                balance: self.acct_balance,
                limit: self.acct_limit,
            });
            self.acct_name.clear();
            self.acct_balance = 0.0;
            changed = true;
        }
        if let Some(i) = remove {
            self.state.accounts.remove(i);
            changed = true;
        }
        if changed {
            self.state.refresh_net_worth();
        }
    }

    pub(super) fn page_savings(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Savings goals",
            "Set a goal and money moves into it automatically: a fixed amount, or Smart Savings that picks the amount from your spending.",
        );
        let ctx = ui.ctx().clone();
        let total: f64 = self.state.goals.iter().map(|g| g.saved).sum();
        let target: f64 = self.state.goals.iter().map(|g| g.target).sum();
        let per_transfer: Vec<f64> = self
            .state
            .goals
            .iter()
            .map(|g| if g.smart { self.state.smart_amount(g.freq) } else { g.auto_amount })
            .collect();
        let monthly: f64 = self
            .state
            .goals
            .iter()
            .zip(&per_transfer)
            .filter(|(g, _)| g.auto_on)
            .map(|(g, a)| a * g.freq.per_month())
            .sum();
        let checking = self.state.checking_balance();
        let threshold = self.state.alerts.low_balance_threshold;
        ui.columns(3, |c| {
            stat(
                &mut c[0],
                "Saved toward goals",
                &money0(total),
                &format!("of {} total", money0(target)),
                GREEN,
            );
            stat(&mut c[1], "Auto-saving each month", &money0(monthly), "from checking", MUTED);
            stat(
                &mut c[2],
                "Checking balance",
                &money(checking),
                &format!("transfers pause under {}", money0(threshold)),
                BLUE,
            );
        });
        ui.add_space(6.0);

        enum Act {
            Deposit(usize, f64),
            Withdraw(usize, f64),
            Remove(usize),
        }
        let mut act = None;
        let colors = [GREEN, BLUE, PURPLE, AMBER, ACCENT];
        let n = self.state.goals.len();
        let per_row = 3;
        for row in 0..n.div_ceil(per_row) {
            ui.columns(per_row, |c| {
                for (k, col_ui) in c.iter_mut().enumerate() {
                    let i = row * per_row + k;
                    if i >= n {
                        break;
                    }
                    let g = &mut self.state.goals[i];
                    let col = colors[i % colors.len()];
                    let amount_now = per_transfer[i];
                    card(col_ui, |ui| {
                        row_right(
                            ui,
                            |ui| {
                                ui.label(RichText::new(g.icon.as_str()).size(22.0).color(col));
                                ui.label(RichText::new(g.name.as_str()).size(17.0).strong());
                            },
                            |ui| {
                                if ui
                                    .small_button("✖")
                                    .on_hover_text("Delete goal (money goes back to checking)")
                                    .clicked()
                                {
                                    act = Some(Act::Remove(i));
                                }
                            },
                        );
                        let f = g.saved / g.target.max(1.0);
                        row_right(
                            ui,
                            |ui| {
                                ui.label(RichText::new(money0(g.saved)).size(22.0).strong());
                                muted(ui, format!("of {}", money0(g.target)));
                            },
                            |ui| {
                                ui.label(RichText::new(format!("{:.0}%", f * 100.0)).color(col).strong());
                            },
                        );
                        progress(ui, f, col, 10.0);
                        let per_month = amount_now * g.freq.per_month();
                        if g.saved >= g.target {
                            chip(ui, "Goal reached 🎉", GREEN);
                        } else if g.auto_on && per_month > 0.0 {
                            let months = ((g.target - g.saved) / per_month).ceil() as i32;
                            muted(
                                ui,
                                format!("On pace to finish around {}", add_months(today(), months).format("%B %Y")),
                            );
                        } else {
                            muted(ui, "Turn on automatic transfers to see when you'll finish.");
                        }
                        ui.separator();
                        ui.checkbox(&mut g.auto_on, "Automatic transfers");
                        ui.add_enabled_ui(g.auto_on, |ui| {
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut g.smart, true, "Smart");
                                ui.selectable_value(&mut g.smart, false, "Fixed amount");
                            });
                            ui.horizontal(|ui| {
                                if g.smart {
                                    ui.label(RichText::new(format!("≈ {}", money0(amount_now))).strong());
                                } else {
                                    money_field(ui, &mut g.auto_amount);
                                }
                                egui::ComboBox::new(format!("freq{}", g.id), "")
                                    .selected_text(g.freq.name())
                                    .show_ui(ui, |ui| {
                                        for f in Freq::ALL {
                                            ui.selectable_value(&mut g.freq, f, f.name());
                                        }
                                    });
                            });
                            if g.smart {
                                muted(ui, "Smart Savings sets aside part of what you didn't spend last month.");
                            }
                            muted(
                                ui,
                                format!("Next transfer {} ({})", g.next_transfer.format("%b %-d"), in_days(g.next_transfer)),
                            );
                        });
                        ui.horizontal(|ui| {
                            for amt in [25.0, 100.0] {
                                if ui.button(format!("+{}", money0(amt))).clicked() {
                                    act = Some(Act::Deposit(i, amt));
                                }
                            }
                            if ui.button("Withdraw $50").clicked() {
                                act = Some(Act::Withdraw(i, 50.0));
                            }
                        });
                    });
                }
            });
            ui.add_space(6.0);
        }

        match act {
            Some(Act::Deposit(i, amt)) => {
                let from = self.state.goals[i].from_account;
                if let Some(a) = self.state.accounts.iter_mut().find(|a| a.id == from) {
                    a.balance -= amt;
                }
                let g = &mut self.state.goals[i];
                g.saved += amt;
                let name = g.name.clone();
                self.state.transfers.push(GoalTransfer {
                    date: today(),
                    goal: name.clone(),
                    amount: amt,
                    note: "Manual transfer".into(),
                });
                self.toast(&ctx, format!("Moved {} into {}.", money0(amt), name));
            }
            Some(Act::Withdraw(i, amt)) => {
                let g = &mut self.state.goals[i];
                let amt = amt.min(g.saved);
                g.saved -= amt;
                let (from, name) = (g.from_account, g.name.clone());
                if let Some(a) = self.state.accounts.iter_mut().find(|a| a.id == from) {
                    a.balance += amt;
                }
                self.state.transfers.push(GoalTransfer {
                    date: today(),
                    goal: name,
                    amount: -amt,
                    note: "Withdrawal".into(),
                });
            }
            Some(Act::Remove(i)) => {
                let g = self.state.goals.remove(i);
                if let Some(a) = self.state.accounts.iter_mut().find(|a| a.id == g.from_account) {
                    a.balance += g.saved;
                }
            }
            None => {}
        }

        let mut create = false;
        let mut run = false;
        ui.columns(2, |c| {
            card(&mut c[0], |ui| {
                section(ui, "New goal");
                egui::Grid::new("newgoal").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut self.goal_name).hint_text(hint("e.g. House deposit")));
                    ui.end_row();
                    ui.label("Target");
                    money_field(ui, &mut self.goal_target);
                    ui.end_row();
                    ui.label("Auto-save");
                    ui.horizontal(|ui| {
                        money_field(ui, &mut self.goal_auto);
                        egui::ComboBox::new("goal_freq", "")
                            .selected_text(self.goal_freq.name())
                            .show_ui(ui, |ui| {
                                for f in Freq::ALL {
                                    ui.selectable_value(&mut self.goal_freq, f, f.name());
                                }
                            });
                    });
                    ui.end_row();
                });
                if primary(ui, "Create goal").clicked() && !self.goal_name.trim().is_empty() && self.goal_target > 0.0 {
                    create = true;
                }
            });
            card(&mut c[1], |ui| {
                row_right(
                    ui,
                    |ui| section(ui, "Transfer history"),
                    |ui| {
                        if ui.button("Run due transfers").clicked() {
                            run = true;
                        }
                    },
                );
                if self.state.transfers.is_empty() {
                    muted(ui, "No transfers yet. The next automatic one is shown on each goal.");
                }
                for tr in self.state.transfers.iter().rev().take(8) {
                    row_right(
                        ui,
                        |ui| {
                            muted(ui, tr.date.format("%b %-d").to_string());
                            ui.label(tr.goal.as_str());
                            muted(ui, tr.note.as_str());
                        },
                        |ui| {
                            ui.label(
                                RichText::new(money(tr.amount))
                                    .color(if tr.amount > 0.0 { GREEN } else { MUTED })
                                    .strong(),
                            );
                        },
                    );
                }
            });
        });
        if create {
            let id = self.state.goals.iter().map(|g| g.id).max().unwrap_or(0) + 1;
            let from = self.state.checking_id();
            self.state.goals.push(Goal {
                id,
                name: self.goal_name.trim().to_string(),
                icon: "★".into(),
                target: self.goal_target,
                saved: 0.0,
                auto_on: self.goal_auto > 0.0,
                smart: false,
                auto_amount: self.goal_auto,
                freq: self.goal_freq,
                next_transfer: self.goal_freq.advance(today()),
                from_account: from,
            });
            self.goal_name.clear();
        }
        if run {
            let notes = self.state.run_due_transfers();
            if notes.is_empty() {
                self.toast(&ctx, "All automatic transfers are up to date.");
            }
            for n in notes {
                self.toast(&ctx, n);
            }
        }
        self.state.refresh_net_worth();
    }
}
