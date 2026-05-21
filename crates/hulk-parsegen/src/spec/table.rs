use crate::production::Production;
use crate::spec::error::GrammarError;
use crate::spec::first::{FirstSets, first_of_sequence};
use crate::spec::follow::FollowSets;
use crate::spec::grammar_spec::GrammarSpec;
use crate::symbol::Symbol;
use std::collections::HashMap;

pub type ParseTable = HashMap<(String, String), Production>;

pub fn build_ll1_table(
    grammar: &GrammarSpec,
    first_sets: &FirstSets,
    follow_sets: &FollowSets,
) -> Result<ParseTable, GrammarError> {
    let mut table = ParseTable::new();

    for production in &grammar.productions {
        let first_alpha = first_of_sequence(&production.rhs, first_sets);

        for symbol in &first_alpha {
            if *symbol == Symbol::Epsilon {
                continue;
            }

            let terminal = terminal_key(symbol).expect("FIRST entries must be terminal-like");
            insert_or_conflict(&mut table, &production.lhs, &terminal, production)?;
        }

        if first_alpha.contains(&Symbol::Epsilon) {
            let follow = follow_sets
                .get(&production.lhs)
                .cloned()
                .unwrap_or_default();
            for symbol in follow {
                let terminal = terminal_key(&symbol).expect("FOLLOW entries must be terminal-like");
                insert_or_conflict(&mut table, &production.lhs, &terminal, production)?;
            }
        }
    }

    Ok(table)
}

fn insert_or_conflict(
    table: &mut ParseTable,
    non_terminal: &str,
    terminal: &str,
    production: &Production,
) -> Result<(), GrammarError> {
    let key = (non_terminal.to_string(), terminal.to_string());
    if let Some(existing) = table.get(&key) {
        if existing != production {
            return Err(GrammarError::Ll1Conflict {
                non_terminal: non_terminal.to_string(),
                terminal: terminal.to_string(),
                existing: existing.clone(),
                incoming: production.clone(),
            });
        }
        return Ok(());
    }

    table.insert(key, production.clone());
    Ok(())
}

pub fn terminal_key(symbol: &Symbol) -> Option<String> {
    match symbol {
        Symbol::Terminal(name) => Some(name.clone()),
        Symbol::Eof => Some("EOF".to_string()),
        Symbol::Epsilon | Symbol::NonTerminal(_) => None,
    }
}
