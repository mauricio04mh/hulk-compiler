pub mod ast;
pub mod builder;
pub mod error;

use ast::Program;
use builder::AstBuilder;
use error::{FrontendError, ParseErrorList};
use hulk_lexgen::lx::lexer::LxLexer;
use hulk_lexgen::lx::parser::LxParser;
use hulk_lexgen::runtime::lexer::lex_hulk;
use hulk_lexgen::spec::lexer_spec::LexerSpec;
use hulk_lexgen::spec::normalize::normalize_spec;
use hulk_parsegen::gx::parser::parse_gx;
use hulk_parsegen::runtime::parser::RuntimeParser;
use hulk_parsegen::runtime::pratt::{Associativity, OperatorInfo, PrattConfig, PrattParser};
use hulk_parsegen::runtime::token::ParseToken;
use hulk_parsegen::spec::first::compute_first_sets;
use hulk_parsegen::spec::follow::compute_follow_sets;
use hulk_parsegen::spec::normalize::normalize_grammar;
use hulk_parsegen::spec::table::build_ll1_table;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Cached pipelines — grammar + LL(1) table + Pratt parser + lexer spec are
// built once per grammar variant and reused for every subsequent parse call.
// ---------------------------------------------------------------------------

struct CachedPipeline {
    runtime: RuntimeParser,
    lexer_spec: LexerSpec,
}

// RuntimeParser and LexerSpec contain only owned String/HashMap/Vec data and are
// Send + Sync automatically. The Result wrapper is also Send + Sync.
static PIPELINE_EXPR: OnceLock<Result<CachedPipeline, String>> = OnceLock::new();
static PIPELINE_FUNCTIONS: OnceLock<Result<CachedPipeline, String>> = OnceLock::new();
static PIPELINE_CONTROL: OnceLock<Result<CachedPipeline, String>> = OnceLock::new();
static PIPELINE_TYPES: OnceLock<Result<CachedPipeline, String>> = OnceLock::new();
static PIPELINE_FULL: OnceLock<Result<CachedPipeline, String>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn parse_hulk_expr_program(source: &str) -> Result<Program, FrontendError> {
    let pipeline = get_pipeline(&PIPELINE_EXPR, || {
        build_pipeline(
            "grammars/hulk_expr.gx",
            "specs/hulk_expr.lx",
            expr_stop_tokens(),
        )
    })?;
    run_pipeline(source, pipeline)
}

pub fn parse_hulk_functions_program(source: &str) -> Result<Program, FrontendError> {
    let pipeline = get_pipeline(&PIPELINE_FUNCTIONS, || {
        build_pipeline(
            "grammars/hulk_functions.gx",
            "specs/hulk_functions.lx",
            functions_stop_tokens(),
        )
    })?;
    run_pipeline(source, pipeline)
}

pub fn parse_hulk_control_program(source: &str) -> Result<Program, FrontendError> {
    let pipeline = get_pipeline(&PIPELINE_CONTROL, || {
        build_pipeline(
            "grammars/hulk_control.gx",
            "specs/hulk_control.lx",
            control_stop_tokens(),
        )
    })?;
    run_pipeline(source, pipeline)
}

pub fn parse_hulk_types_program(source: &str) -> Result<Program, FrontendError> {
    let pipeline = get_pipeline(&PIPELINE_TYPES, || {
        build_pipeline(
            "grammars/hulk_types.gx",
            "specs/hulk_types.lx",
            full_stop_tokens(),
        )
    })?;
    run_pipeline(source, pipeline)
}

pub fn parse_hulk_full_program(source: &str) -> Result<Program, FrontendError> {
    let pipeline = get_pipeline(&PIPELINE_FULL, || {
        build_pipeline(
            "grammars/hulk_full.gx",
            "specs/hulk_full.lx",
            full_stop_tokens(),
        )
    })?;
    run_pipeline(source, pipeline)
}

fn get_pipeline<F>(
    cell: &'static OnceLock<Result<CachedPipeline, String>>,
    init: F,
) -> Result<&'static CachedPipeline, FrontendError>
where
    F: FnOnce() -> Result<CachedPipeline, FrontendError>,
{
    cell.get_or_init(|| init().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| FrontendError::Io(e.clone()))
}

// ---------------------------------------------------------------------------
// Pipeline construction (run once, cached in OnceLock)
// ---------------------------------------------------------------------------

fn build_pipeline(
    gx_rel: &str,
    lx_rel: &str,
    stop_tokens: HashSet<String>,
) -> Result<CachedPipeline, FrontendError> {
    let gx_source = read_parsegen_fixture(gx_rel)?;
    let lx_source = read_parsegen_fixture(lx_rel)?;

    let grammar = parse_gx(&gx_source).map_err(|e| FrontendError::GxParse(e.to_string()))?;
    let spec =
        normalize_grammar(grammar).map_err(|e| FrontendError::GrammarNormalize(e.to_string()))?;
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow)
        .map_err(|e| FrontendError::Ll1Table(e.to_string()))?;

    let runtime = RuntimeParser::new(spec, table).with_pratt_hook(
        "OperatorExpr",
        hulk_pratt_parser(),
        stop_tokens,
    );

    let lx_tokens = LxLexer::new(&lx_source)
        .lex_all()
        .map_err(|e| FrontendError::LxLex(e.to_string()))?;
    let rules = LxParser::new(lx_tokens)
        .parse_rules()
        .map_err(|e| FrontendError::LxParse(e.to_string()))?;
    let lexer_spec =
        normalize_spec(&rules).map_err(|e| FrontendError::LxNormalize(e.to_string()))?;

    Ok(CachedPipeline {
        runtime,
        lexer_spec,
    })
}

// ---------------------------------------------------------------------------
// Parse execution (called on every source input)
// ---------------------------------------------------------------------------

fn run_pipeline(source: &str, pipeline: &CachedPipeline) -> Result<Program, FrontendError> {
    let lex_tokens = lex_hulk(source, &pipeline.lexer_spec)
        .map_err(|e| FrontendError::SourceLex(e.to_string()))?;
    let parse_tokens = lex_tokens
        .into_iter()
        .map(|token| ParseToken {
            kind: token.kind,
            lexeme: token.lexeme,
            line: token.line,
            column: token.column,
        })
        .collect::<Vec<_>>();

    let cst = pipeline
        .runtime
        .parse(&parse_tokens)
        .map_err(|errors| FrontendError::ParseErrors(ParseErrorList(errors)))?;

    Ok(AstBuilder::build_program(&cst)?)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_parsegen_fixture(relative: &str) -> Result<String, FrontendError> {
    let path = format!(
        "{}/../hulk-parsegen/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        relative
    );
    std::fs::read_to_string(path).map_err(|e| FrontendError::Io(e.to_string()))
}

fn hulk_pratt_parser() -> PrattParser {
    let mut binary_ops = HashMap::new();
    binary_ops.insert(
        "ASSIGN".to_string(),
        OperatorInfo {
            precedence: 0,
            associativity: Associativity::Right,
        },
    );
    binary_ops.insert(
        "OR".to_string(),
        OperatorInfo {
            precedence: 1,
            associativity: Associativity::Left,
        },
    );
    binary_ops.insert(
        "AND".to_string(),
        OperatorInfo {
            precedence: 2,
            associativity: Associativity::Left,
        },
    );
    for op in ["EQ", "NEQ"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 3,
                associativity: Associativity::Left,
            },
        );
    }
    for op in ["LT", "LE", "GT", "GE"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 4,
                associativity: Associativity::Left,
            },
        );
    }
    for op in ["AT", "ATAT"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 5,
                associativity: Associativity::Left,
            },
        );
    }
    for op in ["PLUS", "MINUS"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 6,
                associativity: Associativity::Left,
            },
        );
    }
    for op in ["STAR", "SLASH", "MOD"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 7,
                associativity: Associativity::Left,
            },
        );
    }
    binary_ops.insert(
        "POW".to_string(),
        OperatorInfo {
            precedence: 8,
            associativity: Associativity::Right,
        },
    );

    let unary_prefix_ops = ["NOT", "MINUS", "PLUS"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let primary_tokens = ["NUMBER", "IDENT", "STRING", "TRUE", "FALSE"]
        .into_iter()
        .map(str::to_string)
        .collect();

    PrattParser::new(PrattConfig {
        binary_ops,
        unary_prefix_ops,
        primary_tokens,
        lparen: "LPAREN".to_string(),
        rparen: "RPAREN".to_string(),
        comma: Some("COMMA".to_string()),
        new_kw: Some("NEW".to_string()),
        self_kw: Some("SELF".to_string()),
        base_kw: Some("BASE".to_string()),
        dot: Some("DOT".to_string()),
        is_kw: Some("IS".to_string()),
        as_kw: Some("AS".to_string()),
        lbracket: Some("LBRACKET".to_string()),
        rbracket: Some("RBRACKET".to_string()),
        arrow: Some("ARROW".to_string()),
        funcarrow: Some("FUNCARROW".to_string()),
        if_kw: Some("IF".to_string()),
        elif_kw: Some("ELIF".to_string()),
        else_kw: Some("ELSE".to_string()),
        while_kw: Some("WHILE".to_string()),
        for_kw: Some("FOR".to_string()),
        in_kw: Some("IN".to_string()),
        lbrace: Some("LBRACE".to_string()),
        rbrace: Some("RBRACE".to_string()),
        semicolon: Some("SEMICOLON".to_string()),
        function_kw: Some("FUNCTION".to_string()),
        let_kw: Some("LET".to_string()),
        match_kw: Some("MATCH".to_string()),
        wildcard: Some("WILDCARD".to_string()),
    })
}

fn expr_stop_tokens() -> HashSet<String> {
    ["SEMICOLON", "COMMA", "RPAREN", "IN", "EOF"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn functions_stop_tokens() -> HashSet<String> {
    ["SEMICOLON", "COMMA", "RPAREN", "IN", "RBRACE", "EOF"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn control_stop_tokens() -> HashSet<String> {
    [
        "SEMICOLON",
        "COMMA",
        "RPAREN",
        "IN",
        "RBRACE",
        "ELIF",
        "ELSE",
        "EOF",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn full_stop_tokens() -> HashSet<String> {
    [
        "SEMICOLON",
        "COMMA",
        "RPAREN",
        "IN",
        "RBRACE",
        "ELIF",
        "ELSE",
        "EOF",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}
