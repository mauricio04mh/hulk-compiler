use crate::spec::first::{FirstSets, first_of_sequence};
use crate::spec::grammar_spec::GrammarSpec;
use crate::symbol::Symbol;
use std::collections::{HashMap, HashSet};

pub type FollowSets = HashMap<String, HashSet<Symbol>>;

pub fn compute_follow_sets(grammar: &GrammarSpec, first_sets: &FirstSets) -> FollowSets {
    let mut follow_sets = HashMap::<String, HashSet<Symbol>>::new();

    for non_terminal in &grammar.non_terminals {
        follow_sets.entry(non_terminal.clone()).or_default();
    }

    follow_sets
        .entry(grammar.start.clone())
        .or_default()
        .insert(Symbol::Eof);

    let mut changed = true;
    while changed {
        changed = false;

        for production in &grammar.productions {
            for (idx, symbol) in production.rhs.iter().enumerate() {
                let Symbol::NonTerminal(current_non_terminal) = symbol else {
                    continue;
                };

                let beta = &production.rhs[idx + 1..];
                let beta_first = first_of_sequence(beta, first_sets);

                {
                    let target_follow =
                        follow_sets.entry(current_non_terminal.clone()).or_default();
                    for first_symbol in &beta_first {
                        if *first_symbol != Symbol::Epsilon
                            && target_follow.insert(first_symbol.clone())
                        {
                            changed = true;
                        }
                    }
                }

                if beta.is_empty() || beta_first.contains(&Symbol::Epsilon) {
                    let lhs_follow = follow_sets
                        .get(&production.lhs)
                        .cloned()
                        .unwrap_or_default();
                    let target_follow =
                        follow_sets.entry(current_non_terminal.clone()).or_default();

                    for follow_symbol in lhs_follow {
                        if target_follow.insert(follow_symbol) {
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    follow_sets
}
