//! The money assistant: answers questions from the person's own data, on this computer.

use crate::data::*;

pub const SUGGESTIONS: [&str; 6] = [
    "Where did my money go this month?",
    "How much do I spend on subscriptions?",
    "Am I on track with my budgets?",
    "How can I save more?",
    "What's my net worth?",
    "Can I afford $400?",
];

fn pct(a: f64, b: f64) -> String {
    if b.abs() < 0.01 {
        "n/a".into()
    } else {
        format!("{:.0}%", a / b * 100.0)
    }
}

pub fn answer(s: &State, question: &str) -> String {
    let q = question.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| q.contains(w));
    let t = today();
    let last = add_months(t, -1);

    // A question about one category.
    for c in Category::all().into_iter().filter(|c| c.is_spending()) {
        let short = c.name().split(' ').next().unwrap_or("").to_lowercase();
        let extra: &[&str] = match c {
            Category::Dining => &["eating", "eat out", "restaurant", "food", "coffee"],
            Category::Transport => &["gas", "fuel", "my car", "rideshare", "transport"],
            Category::Utilities => &["utilit", "electric"],
            Category::Groceries => &["grocer"],
            _ => &[],
        };
        let short_ok = short.len() > 3 && !matches!(c, Category::Subscriptions | Category::Utilities | Category::Other);
        if (short_ok && q.contains(&short)) || extra.iter().any(|w| q.contains(w)) {
            let now = s.spent_in(t, Some(c));
            let before = s.spent_in(last, Some(c));
            let mut out = format!(
                "You've spent {} on {} so far this month, compared with {} all of last month.",
                money(now),
                c.name(),
                money(before)
            );
            if let Some(b) = s.budgets.iter().find(|b| b.category == c) {
                let left = b.limit - now;
                if left >= 0.0 {
                    out += &format!(
                        " That's {} of your {} budget, so {} is left.",
                        pct(now, b.limit),
                        money0(b.limit),
                        money(left)
                    );
                } else {
                    out += &format!(" You're {} over your {} budget.", money(-left), money0(b.limit));
                }
            }
            let mut top: Vec<(String, f64)> = Vec::new();
            for tx in s.in_month(t).filter(|x| x.category == c && x.amount < 0.0) {
                match top.iter_mut().find(|e| e.0 == tx.merchant) {
                    Some(e) => e.1 += -tx.amount,
                    None => top.push((tx.merchant.clone(), -tx.amount)),
                }
            }
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            if !top.is_empty() {
                let list: Vec<String> = top.iter().take(3).map(|(m, v)| format!("{} ({})", m, money(*v))).collect();
                out += &format!(" Most of it went to {}.", list.join(", "));
            }
            return out;
        }
    }

    if has(&["subscription", "recurring", "cancel"]) {
        let mut subs: Vec<&Subscription> = s.active_subs().filter(|x| x.category == Category::Subscriptions).collect();
        subs.sort_by(|a, b| b.monthly_cost().partial_cmp(&a.monthly_cost()).unwrap());
        let total: f64 = subs.iter().map(|x| x.monthly_cost()).sum();
        let mut out = format!(
            "You have {} active subscriptions costing about {} a month ({} a year).",
            subs.len(),
            money(total),
            money(total * 12.0)
        );
        if let Some(first) = subs.first() {
            out += &format!(" The biggest is {} at {}.", first.merchant, money(first.amount));
        }
        if let Some(next) = s.active_subs().min_by_key(|x| x.next_charge()) {
            out += &format!(" Next up: {} on {}.", next.merchant, next.next_charge().format("%b %-d"));
        }
        out += " Open Recurring to cancel any you don't use.";
        return out;
    }

    if has(&["budget", "on track", "overspend"]) {
        if s.budgets.is_empty() {
            return "You haven't set any budgets yet. Open Budgets to add one for each category you want to keep an eye on.".into();
        }
        let mut over = Vec::new();
        let mut close = Vec::new();
        let mut fine = 0;
        for b in &s.budgets {
            let spent = s.spent_in(t, Some(b.category));
            if spent > b.limit {
                over.push(format!("{} ({} over)", b.category.name(), money(spent - b.limit)));
            } else if spent > b.limit * 0.85 {
                close.push(b.category.name().to_string());
            } else {
                fine += 1;
            }
        }
        let total_limit: f64 = s.budgets.iter().map(|b| b.limit).sum();
        let total_spent: f64 = s.budgets.iter().map(|b| s.spent_in(t, Some(b.category))).sum();
        let mut out = format!(
            "You've used {} of your {} total budget this month. ",
            pct(total_spent, total_limit),
            money0(total_limit)
        );
        if !over.is_empty() {
            out += &format!("Over budget: {}. ", over.join(", "));
        }
        if !close.is_empty() {
            out += &format!("Getting close: {}. ", close.join(", "));
        }
        out += &format!("{} categor{} on track.", fine, if fine == 1 { "y is" } else { "ies are" });
        return out;
    }

    if has(&["save", "saving", "goal", "cut"]) {
        let goals: Vec<String> = s
            .goals
            .iter()
            .map(|g| format!("{} is {} funded", g.name, pct(g.saved, g.target)))
            .collect();
        let auto: f64 = s
            .goals
            .iter()
            .filter(|g| g.auto_on)
            .map(|g| (if g.smart { s.smart_amount(g.freq) } else { g.auto_amount }) * g.freq.per_month())
            .sum();
        let dining = s.spent_in(last, Some(Category::Dining));
        let subs = s.subs_monthly_total();
        let mut out = String::new();
        if !goals.is_empty() {
            out += &format!(
                "{}. Automatic transfers put away about {} a month. ",
                goals.join("; "),
                money0(auto)
            );
        }
        out += &format!(
            "Ideas: cutting dining out by a quarter would free about {} a month, and your recurring charges total {} a month, so cancelling one or two unused ones adds up fast.",
            money0(dining * 0.25),
            money(subs)
        );
        let eligible = s
            .bills
            .iter()
            .filter(|b| b.kind.eligible() && b.status == NegStatus::NotStarted)
            .count();
        if eligible > 0 {
            out += &format!(
                " You also have {} bill{} we could try to negotiate down.",
                eligible,
                if eligible == 1 { "" } else { "s" }
            );
        }
        return out;
    }

    if has(&["net worth", "worth", "assets", "debt"]) {
        let nw = s.net_worth();
        let mut out = format!(
            "Your net worth is {}: {} in assets and {} in debts.",
            money(nw),
            money(s.assets()),
            money(s.debts())
        );
        if s.net_worth_history.len() >= 4 {
            let then = s.net_worth_history[s.net_worth_history.len() - 4].1;
            let change = nw - then;
            out += &format!(
                " That's {} {} over the last 3 months.",
                if change >= 0.0 { "up" } else { "down" },
                money(change.abs())
            );
        }
        return out;
    }

    if has(&["credit", "score"]) {
        if !s.credit.has_score() {
            return "There's no credit score yet. Once credit monitoring is connected, I can explain what's moving it.".into();
        }
        let score = s.credit.score();
        let (band, _) = score_band(score);
        let util = s.card_utilization() * 100.0;
        let mut out = format!("Your credit score is {} ({}). Card utilization is {:.0}%.", score, band, util);
        if util > 30.0 {
            out += " Paying cards down below 30% of their limits is the fastest way to raise it.";
        } else {
            out += " Keep paying on time and keeping balances low and it should keep climbing.";
        }
        return out;
    }

    if has(&["afford"]) {
        let amount: f64 = q
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter_map(|w| w.parse::<f64>().ok())
            .next()
            .unwrap_or(0.0);
        let checking = s.checking_balance();
        let upcoming: f64 = s
            .active_subs()
            .filter(|x| x.account == s.checking_id() && (x.next_charge() - t).num_days() <= 30)
            .map(|x| x.amount)
            .sum();
        let free = checking - upcoming - s.alerts.low_balance_threshold;
        if amount <= 0.0 {
            return format!(
                "After the {} in bills due in the next 30 days and keeping your {} cushion, you have about {} free to spend from checking.",
                money(upcoming),
                money0(s.alerts.low_balance_threshold),
                money(free.max(0.0))
            );
        }
        return if amount <= free {
            format!(
                "Yes. After {} of upcoming bills you'd still have {} above your {} low-balance cushion.",
                money(upcoming),
                money(free - amount),
                money0(s.alerts.low_balance_threshold)
            )
        } else {
            format!(
                "It would be tight. Checking has {}, with {} of bills due in the next 30 days. Spending {} would leave you {} below your cushion.",
                money(checking),
                money(upcoming),
                money(amount),
                money(amount - free)
            )
        };
    }

    if has(&["bill", "negotiat", "lower"]) {
        let eligible: Vec<&Bill> = s.bills.iter().filter(|b| b.kind.eligible()).collect();
        let saved: f64 = s
            .bills
            .iter()
            .filter_map(|b| match b.status {
                NegStatus::Saved { new_monthly, .. } => Some((b.monthly - new_monthly) * 12.0),
                _ => None,
            })
            .sum();
        return format!(
            "{} of your bills can be negotiated. So far negotiations have saved you {} a year. Open Bill Negotiation to start one; you only pay {:.0}% of the first year's savings if it works.",
            eligible.len(),
            money(saved),
            s.fee_pct
        );
    }

    if has(&[
        "income",
        "cash flow",
        "earn",
        "left over",
        "spend",
        "where",
        "most",
        "money go",
        "top",
    ]) {
        let income = s.income_in(t);
        let spent = s.spent_in(t, None);
        let cats = s.spending_by_category(t);
        let top: Vec<String> = cats.iter().take(3).map(|(c, v)| format!("{} {}", c.name(), money(*v))).collect();
        return format!(
            "This month you've brought in {} and spent {}, so you're {} {}. Your top categories are {}. Last month you spent {} in total.",
            money(income),
            money(spent),
            if income >= spent { "ahead by" } else { "behind by" },
            money((income - spent).abs()),
            top.join(", "),
            money(s.spent_in(last, None))
        );
    }

    "I can help with your spending, budgets, subscriptions, savings goals, bills, net worth and credit score. Try asking \"How much did I spend on dining?\" or \"Can I afford $250?\"".into()
}
