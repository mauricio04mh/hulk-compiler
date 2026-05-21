use crate::spec::grammar_spec::GrammarSpec;
use crate::symbol::Symbol;
use std::collections::{HashMap, HashSet};

pub type FirstSets = HashMap<String, HashSet<Symbol>>;

pub fn compute_first_sets(grammar: &GrammarSpec) -> FirstSets {
    let mut first_sets = HashMap::<String, HashSet<Symbol>>::new();

    for non_terminal in &grammar.non_terminals {
        first_sets.entry(non_terminal.clone()).or_default();
    }

    let mut changed = true;
    while changed {
        changed = false;

        for production in &grammar.productions {
            let sequence_first = first_of_sequence(&production.rhs, &first_sets);
            let current = first_sets.entry(production.lhs.clone()).or_default();

            for symbol in sequence_first {
                if current.insert(symbol) {
                    changed = true;
                }
            }
        }
    }

    first_sets
}

pub fn first_of_sequence(sequence: &[Symbol], first_sets: &FirstSets) -> HashSet<Symbol> {
    let mut result = HashSet::new();

    if sequence.is_empty() {
        result.insert(Symbol::Epsilon);
        return result;
    }

    let mut all_can_derive_epsilon = true;
    for symbol in sequence {
        match symbol {
            Symbol::Terminal(name) => {
                result.insert(Symbol::Terminal(name.clone()));
                all_can_derive_epsilon = false;
                break;
            }
            Symbol::Eof => {
                result.insert(Symbol::Eof);
                all_can_derive_epsilon = false;
                break;
            }
            Symbol::Epsilon => {
                continue;
            }
            Symbol::NonTerminal(name) => {
                let symbol_first = first_sets.get(name).cloned().unwrap_or_default();
                let has_epsilon = symbol_first.contains(&Symbol::Epsilon);

                for first_symbol in symbol_first {
                    if first_symbol != Symbol::Epsilon {
                        result.insert(first_symbol);
                    }
                }

                if !has_epsilon {
                    all_can_derive_epsilon = false;
                    break;
                }
            }
        }
    }

    if all_can_derive_epsilon {
        result.insert(Symbol::Epsilon);
    }

    result
}
