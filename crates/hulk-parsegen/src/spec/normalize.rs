use crate::grammar::Grammar;
use crate::spec::error::GrammarError;
use crate::spec::grammar_spec::GrammarSpec;
use crate::symbol::Symbol;

pub fn normalize_grammar(grammar: Grammar) -> Result<GrammarSpec, GrammarError> {
    if grammar.productions.is_empty() {
        return Err(GrammarError::EmptyGrammar);
    }

    let non_terminals = grammar.non_terminals();
    if !non_terminals.contains(&grammar.start) {
        return Err(GrammarError::UndefinedStartSymbol(grammar.start));
    }

    for production in &grammar.productions {
        let has_epsilon = production
            .rhs
            .iter()
            .any(|symbol| *symbol == Symbol::Epsilon);
        if has_epsilon && (production.rhs.len() != 1 || production.rhs[0] != Symbol::Epsilon) {
            return Err(GrammarError::InvalidEpsilonUsage {
                lhs: production.lhs.clone(),
            });
        }

        for symbol in &production.rhs {
            if let Symbol::NonTerminal(name) = symbol
                && !non_terminals.contains(name)
            {
                return Err(GrammarError::UndefinedNonTerminal(name.clone()));
            }
        }
    }

    // TODO: future iteration should validate left recursion and transform grammar if needed.
    // TODO: future iteration should check ambiguity patterns beyond LL(1) table conflicts.
    let terminals = grammar.terminals();
    Ok(GrammarSpec {
        start: grammar.start,
        productions: grammar.productions,
        non_terminals,
        terminals,
    })
}
