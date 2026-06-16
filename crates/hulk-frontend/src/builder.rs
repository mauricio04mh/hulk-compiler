use crate::ast::{
    AttributeDecl, BinaryOp, Decl, Expr, FunctionDecl, LetBinding, MethodDecl, Param, Program,
    ProtocolDecl, ProtocolMethod, Span, TypeDecl, TypeMember, TypeParent, TypeRef, UnaryOp,
};
use crate::error::AstError;
use hulk_parsegen::runtime::cst::CstNode;

pub struct AstBuilder;

impl AstBuilder {
    pub fn build_program(cst: &CstNode) -> Result<Program, AstError> {
        let (name, children) = as_node(cst)?;
        if name != "Program" {
            return Err(AstError::UnexpectedNode {
                name: name.to_string(),
                location: AstError::no_location(),
            });
        }

        let mut declarations = Vec::<Decl>::new();
        if let Some(decl_list) = children
            .iter()
            .find(|child| matches!(as_node_name(child), Some("DeclList")))
        {
            Self::collect_decl_list(decl_list, &mut declarations)?;
        }

        let expr_nodes = children
            .iter()
            .filter(|child| matches!(as_node_name(child), Some("Expr")))
            .collect::<Vec<_>>();

        let entry_node = if let Some(node) = expr_nodes.last() {
            *node
        } else {
            children
                .iter()
                .find(|child| matches!(as_node_name(child), Some("Expr")))
                .ok_or_else(|| AstError::MissingChild {
                    node: "Program".to_string(),
                    location: AstError::no_location(),
                })?
        };

        let entry = Self::build_expr(entry_node)?;
        Ok(Program {
            declarations,
            entry,
        })
    }

    fn collect_decl_list(cst: &CstNode, out: &mut Vec<Decl>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(decl_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Decl")))
        {
            out.push(Self::build_decl(decl_node)?);
        }

        if let Some(next) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("DeclList")))
        {
            Self::collect_decl_list(next, out)?;
        }

        Ok(())
    }

    fn build_decl(cst: &CstNode) -> Result<Decl, AstError> {
        let (_, children) = as_node(cst)?;
        if let Some(func_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("FunctionDecl")))
        {
            return Ok(Decl::Function(Self::build_function_decl(func_node)?));
        }

        if let Some(type_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeDecl")))
        {
            return Ok(Decl::Type(Self::build_type_decl(type_node)?));
        }

        if let Some(proto_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ProtocolDecl")))
        {
            return Ok(Decl::Protocol(Self::build_protocol_decl(proto_node)?));
        }

        Err(AstError::MissingChild {
            node: "Decl".to_string(),
            location: AstError::no_location(),
        })
    }

    fn build_function_decl(cst: &CstNode) -> Result<FunctionDecl, AstError> {
        let (_, children) = as_node(cst)?;

        let (name_str, name_span) = children
            .iter()
            .find_map(|node| token_if_kind_with_span(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "FunctionDecl".to_string(),
                location: AstError::no_location(),
            })?;
        let name = name_str.to_string();

        let params = if let Some(param_list_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ParamList")))
        {
            Self::build_param_list(param_list_node)?
        } else {
            Vec::new()
        };

        let return_type = if let Some(return_type_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ReturnType")))
        {
            Self::build_optional_typeref(return_type_node)?
        } else {
            None
        };

        let body_node = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("FunctionBody")))
            .ok_or_else(|| AstError::MissingChild {
                node: "FunctionDecl".to_string(),
                location: AstError::no_location(),
            })?;

        let body = Self::build_function_body(body_node)?;

        Ok(FunctionDecl {
            name,
            name_span,
            params,
            return_type,
            body,
        })
    }

    fn build_type_decl(cst: &CstNode) -> Result<TypeDecl, AstError> {
        let (_, children) = as_node(cst)?;

        let (name_str, name_span) = children
            .iter()
            .find_map(|node| token_if_kind_with_span(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeDecl".to_string(),
                location: AstError::no_location(),
            })?;
        let name = name_str.to_string();

        let params = if let Some(param_list_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeParamList")))
        {
            Self::build_type_param_list(param_list_node)?
        } else {
            Vec::new()
        };

        let parent = if let Some(parent_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeParent")))
        {
            Self::build_type_parent(parent_node)?
        } else {
            None
        };

        let members = if let Some(members_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeMemberList")))
        {
            Self::build_type_member_list(members_node)?
        } else {
            Vec::new()
        };

        Ok(TypeDecl {
            name,
            name_span,
            params,
            parent,
            members,
        })
    }

    fn build_protocol_decl(cst: &CstNode) -> Result<ProtocolDecl, AstError> {
        let (_, children) = as_node(cst)?;

        let (name_str, name_span) = children
            .iter()
            .find_map(|node| token_if_kind_with_span(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "ProtocolDecl".to_string(),
                location: AstError::no_location(),
            })?;
        let name = name_str.to_string();

        let parent = if let Some(parent_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ProtocolParent")))
        {
            Self::build_protocol_parent(parent_node)?
        } else {
            None
        };

        let methods = if let Some(members_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ProtocolMemberList")))
        {
            Self::build_protocol_member_list(members_node)?
        } else {
            Vec::new()
        };

        Ok(ProtocolDecl {
            name,
            name_span,
            parent,
            methods,
        })
    }

    fn build_protocol_parent(cst: &CstNode) -> Result<Option<String>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(None);
        }
        let name = children
            .iter()
            .find_map(|node| token_if_kind(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "ProtocolParent".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();
        Ok(Some(name))
    }

    fn build_protocol_member_list(cst: &CstNode) -> Result<Vec<ProtocolMethod>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(Vec::new());
        }

        let mut methods = Vec::new();
        if let Some(member_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ProtocolMember")))
        {
            methods.push(Self::build_protocol_member(member_node)?);
        }

        if let Some(next) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ProtocolMemberList")))
        {
            methods.extend(Self::build_protocol_member_list(next)?);
        }

        Ok(methods)
    }

    fn build_protocol_member(cst: &CstNode) -> Result<ProtocolMethod, AstError> {
        let (_, children) = as_node(cst)?;

        let name = children
            .iter()
            .find_map(|node| token_if_kind(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "ProtocolMember".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let params = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ParamList")))
            .map(Self::build_param_list)
            .transpose()?
            .unwrap_or_default();

        let return_type = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ReturnType")))
            .map(Self::build_optional_typeref)
            .transpose()?
            .unwrap_or(None);

        Ok(ProtocolMethod {
            name,
            params,
            return_type,
        })
    }

    fn build_type_param_list(cst: &CstNode) -> Result<Vec<Param>, AstError> {
        let (_, children) = as_node(cst)?;
        let Some(param_list) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ParamList")))
        else {
            return Ok(Vec::new());
        };

        Self::build_param_list(param_list)
    }

    fn build_type_parent(cst: &CstNode) -> Result<Option<TypeParent>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(None);
        }

        let parent_name = children
            .iter()
            .find_map(|node| token_if_kind(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeParent".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let args = if let Some(parent_arg_list) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ParentArgList")))
        {
            Some(Self::build_parent_arg_list(parent_arg_list)?)
        } else {
            None // no `(...)` clause → passthrough constructor params
        };

        Ok(Some(TypeParent {
            name: parent_name,
            args,
        }))
    }

    fn build_parent_arg_list(cst: &CstNode) -> Result<Vec<Expr>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(Vec::new());
        }

        let Some(arg_list) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ArgList")))
        else {
            return Ok(Vec::new());
        };

        Self::build_arg_list(arg_list)
    }

    fn build_type_member_list(cst: &CstNode) -> Result<Vec<TypeMember>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(Vec::new());
        }

        let mut members = Vec::new();
        if let Some(member_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeMember")))
        {
            members.push(Self::build_type_member(member_node)?);
        }

        if let Some(next) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeMemberList")))
        {
            members.extend(Self::build_type_member_list(next)?);
        }

        Ok(members)
    }

    fn build_type_member(cst: &CstNode) -> Result<TypeMember, AstError> {
        let (_, children) = as_node(cst)?;

        let name = children
            .iter()
            .find_map(|node| token_if_kind(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeMember".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let tail = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeMemberTail")))
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeMember".to_string(),
                location: AstError::no_location(),
            })?;

        let (_, tail_children) = as_node(tail)?;

        if tail_children
            .iter()
            .any(|n| matches!(as_node_name(n), Some("FunctionBody")))
        {
            let params = tail_children
                .iter()
                .find(|node| matches!(as_node_name(node), Some("ParamList")))
                .map(Self::build_param_list)
                .transpose()?
                .unwrap_or_default();

            let return_type = tail_children
                .iter()
                .find(|node| matches!(as_node_name(node), Some("ReturnType")))
                .map(Self::build_optional_typeref)
                .transpose()?
                .unwrap_or(None);

            let body_node = tail_children
                .iter()
                .find(|node| matches!(as_node_name(node), Some("FunctionBody")))
                .ok_or_else(|| AstError::MissingChild {
                    node: "TypeMemberTail".to_string(),
                    location: AstError::no_location(),
                })?;
            let body = Self::build_function_body(body_node)?;

            return Ok(TypeMember::Method(MethodDecl {
                name,
                params,
                return_type,
                body,
            }));
        }

        let ty = tail_children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeAnnotation")))
            .map(Self::build_optional_typeref)
            .transpose()?
            .unwrap_or(None);

        let expr_node = tail_children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeMemberTail".to_string(),
                location: AstError::no_location(),
            })?;
        let value = Self::build_expr(expr_node)?;

        Ok(TypeMember::Attribute(AttributeDecl { name, ty, value }))
    }

    fn build_param_list(cst: &CstNode) -> Result<Vec<Param>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        if let Some(first_param) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Param")))
        {
            params.push(Self::build_param(first_param)?);
        }

        if let Some(tail) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ParamListTail")))
        {
            Self::collect_param_tail(tail, &mut params)?;
        }

        Ok(params)
    }

    fn collect_param_tail(cst: &CstNode, out: &mut Vec<Param>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(param) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Param")))
        {
            out.push(Self::build_param(param)?);
        }

        if let Some(next) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ParamListTail")))
        {
            Self::collect_param_tail(next, out)?;
        }

        Ok(())
    }

    fn build_param(cst: &CstNode) -> Result<Param, AstError> {
        let (_, children) = as_node(cst)?;
        let name = children
            .iter()
            .find_map(|node| token_if_kind(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "Param".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let ty = if let Some(type_annotation) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeAnnotation")))
        {
            Self::build_optional_typeref(type_annotation)?
        } else {
            None
        };

        Ok(Param { name, ty })
    }

    /// Build an optional TypeRef from a TypeAnnotation or ReturnType CST node.
    fn build_optional_typeref(cst: &CstNode) -> Result<Option<TypeRef>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(None);
        }
        // New grammar: TypeAnnotation -> COLON TypeExpr
        if let Some(type_expr_node) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("TypeExpr")))
        {
            return Ok(Some(Self::build_typeref_from_typeexpr(type_expr_node)?));
        }
        // Legacy grammar (hulk_functions/control): TypeAnnotation -> COLON IDENT
        if let Some(name) = children.iter().find_map(|n| token_if_kind(n, "IDENT")) {
            return Ok(Some(TypeRef::Simple(name.to_string())));
        }
        Ok(None)
    }

    /// Build a TypeRef from a `TypeExpr` grammar node.
    fn build_typeref_from_typeexpr(cst: &CstNode) -> Result<TypeRef, AstError> {
        let (_, children) = as_node(cst)?;

        // TypeExpr -> IDENT TypeExprSuffix
        if let Some(ident) = children.iter().find_map(|n| token_if_kind(n, "IDENT")) {
            let base = TypeRef::Simple(ident.to_string());
            if let Some(suffix) = children
                .iter()
                .find(|n| matches!(as_node_name(n), Some("TypeExprSuffix")))
            {
                let (_, suffix_children) = as_node(suffix)?;
                if suffix_children.is_empty() {
                    return Ok(base);
                }
                if let Some(first) = suffix_children.first() {
                    match as_token_kind(first) {
                        Some("STAR") => return Ok(TypeRef::Iterable(Box::new(base))),
                        Some("LBRACKET") => return Ok(TypeRef::Vector(Box::new(base))),
                        _ => return Ok(base),
                    }
                }
            }
            return Ok(base);
        }

        // TypeExpr -> LPAREN FunctorTypeParams RPAREN FUNCARROW TypeExpr
        let params = if let Some(params_node) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("FunctorTypeParams")))
        {
            Self::build_functor_type_params(params_node)?
        } else {
            Vec::new()
        };

        let ret_node = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("TypeExpr")))
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeExpr(functor)".to_string(),
                location: AstError::no_location(),
            })?;
        let ret = Self::build_typeref_from_typeexpr(ret_node)?;

        Ok(TypeRef::Functor {
            params,
            ret: Box::new(ret),
        })
    }

    fn build_functor_type_params(cst: &CstNode) -> Result<Vec<TypeRef>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        if let Some(first) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("TypeExpr")))
        {
            params.push(Self::build_typeref_from_typeexpr(first)?);
        }

        if let Some(tail) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("FunctorTypeParamsTail")))
        {
            Self::collect_functor_type_params_tail(tail, &mut params)?;
        }

        Ok(params)
    }

    fn collect_functor_type_params_tail(
        cst: &CstNode,
        out: &mut Vec<TypeRef>,
    ) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(type_expr) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("TypeExpr")))
        {
            out.push(Self::build_typeref_from_typeexpr(type_expr)?);
        }

        if let Some(next_tail) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("FunctorTypeParamsTail")))
        {
            Self::collect_functor_type_params_tail(next_tail, out)?;
        }

        Ok(())
    }

    /// Build a TypeRef from a Pratt-generated type node (TypeVector, TypeIterable, or IDENT token).
    fn build_typeref_from_pratt_node(cst: &CstNode) -> Result<TypeRef, AstError> {
        match cst {
            CstNode::Token { kind, lexeme, .. } if kind == "IDENT" => {
                Ok(TypeRef::Simple(lexeme.clone()))
            }
            CstNode::Node { name, children } => match name.as_str() {
                "TypeVector" => {
                    let inner = children
                        .iter()
                        .find_map(|n| token_if_kind(n, "IDENT"))
                        .ok_or_else(|| AstError::MissingChild {
                            node: "TypeVector".to_string(),
                            location: AstError::no_location(),
                        })?
                        .to_string();
                    Ok(TypeRef::Vector(Box::new(TypeRef::Simple(inner))))
                }
                "TypeIterable" => {
                    let inner = children
                        .iter()
                        .find_map(|n| token_if_kind(n, "IDENT"))
                        .ok_or_else(|| AstError::MissingChild {
                            node: "TypeIterable".to_string(),
                            location: AstError::no_location(),
                        })?
                        .to_string();
                    Ok(TypeRef::Iterable(Box::new(TypeRef::Simple(inner))))
                }
                "TypeFunctor" => {
                    // Last child is TypeFunctorReturn wrapping the return type.
                    // All preceding children are parameter types.
                    let ret_node = children.last().ok_or_else(|| AstError::MissingChild {
                        node: "TypeFunctor".to_string(),
                        location: AstError::no_location(),
                    })?;
                    let (ret_name, ret_children) = as_node(ret_node)?;
                    if ret_name != "TypeFunctorReturn" {
                        return Err(AstError::UnexpectedNode {
                            name: ret_name.to_string(),
                            location: AstError::no_location(),
                        });
                    }
                    let ret_type_node =
                        ret_children.first().ok_or_else(|| AstError::MissingChild {
                            node: "TypeFunctorReturn".to_string(),
                            location: AstError::no_location(),
                        })?;
                    let ret = Self::build_typeref_from_pratt_node(ret_type_node)?;

                    let params = children[..children.len() - 1]
                        .iter()
                        .map(Self::build_typeref_from_pratt_node)
                        .collect::<Result<Vec<_>, _>>()?;

                    Ok(TypeRef::Functor {
                        params,
                        ret: Box::new(ret),
                    })
                }
                other => Err(AstError::UnsupportedConstruct {
                    message: format!("Unsupported type node '{}'", other),
                    location: AstError::no_location(),
                }),
            },
            other => Err(AstError::UnsupportedConstruct {
                message: format!("Expected type node, got {:?}", as_token_kind(other)),
                location: AstError::no_location(),
            }),
        }
    }

    fn build_function_body(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;

        if let Some(expr_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
        {
            return Self::build_expr(expr_node);
        }

        if let Some(block_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("BlockExpr")))
        {
            return Self::build_block_expr(block_node);
        }

        Err(AstError::MissingChild {
            node: "FunctionBody".to_string(),
            location: AstError::no_location(),
        })
    }

    fn build_expr(cst: &CstNode) -> Result<Expr, AstError> {
        match cst {
            CstNode::Error {
                message,
                line,
                column,
            } => Err(AstError::UnsupportedConstruct {
                message: message.clone(),
                location: AstError::at(*line, *column),
            }),
            CstNode::Token { kind, lexeme, .. } => Self::build_primary_token(kind, lexeme, cst),
            CstNode::Node { name, children } => match name.as_str() {
                "Expr" | "OperatorExpr" => {
                    let child = children.first().ok_or_else(|| AstError::MissingChild {
                        node: name.clone(),
                        location: AstError::no_location(),
                    })?;
                    Self::build_expr(child)
                }
                "IfExpr" => Self::build_if_expr(cst),
                "WhileExpr" => Self::build_while_expr(cst),
                "ForExpr" => Self::build_for_expr(cst),
                "LetExpr" => Self::build_let_expr(cst),
                "BlockExpr" => Self::build_block_expr(cst),
                "BinaryExpr" => Self::build_binary_expr(cst),
                "UnaryExpr" => Self::build_unary_expr(cst),
                "CallExpr" => Self::build_call_expr(cst),
                "NewExpr" => Self::build_new_expr(cst),
                "SelfExpr" => Ok(Expr::SelfRef),
                "BaseCallExpr" => Self::build_base_call_expr(cst),
                "MemberAccessExpr" => Self::build_member_access_expr(cst),
                "MethodCallExpr" => Self::build_method_call_expr(cst),
                "TypeTestExpr" => Self::build_type_test_expr(cst),
                "TypeCastExpr" => Self::build_type_cast_expr(cst),
                "VectorLiteralExpr" => Self::build_vector_literal_expr(cst),
                "VectorGeneratorExpr" => Self::build_vector_generator_expr(cst),
                "VectorIndexExpr" => Self::build_vector_index_expr(cst),
                "LambdaExpr" => Self::build_lambda_expr(cst),
                "GroupExpr" => {
                    let child = children.first().ok_or_else(|| AstError::MissingChild {
                        node: "GroupExpr".to_string(),
                        location: AstError::no_location(),
                    })?;
                    Self::build_expr(child)
                }
                _ if children.len() == 1 => Self::build_expr(&children[0]),
                other => Err(AstError::UnsupportedConstruct {
                    message: format!("Cannot build Expr from node '{}'", other),
                    location: AstError::no_location(),
                }),
            },
        }
    }

    fn build_new_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        let type_name = children
            .first()
            .and_then(|n| token_if_kind(n, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "NewExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let mut args = Vec::new();
        for arg in children.iter().skip(1) {
            args.push(Self::build_expr(arg)?);
        }

        Ok(Expr::New {
            span,
            type_name,
            args,
        })
    }

    fn build_base_call_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        let mut args = Vec::new();
        for arg in children {
            args.push(Self::build_expr(arg)?);
        }
        Ok(Expr::BaseCall { span, args })
    }

    fn build_member_access_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        if children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "MemberAccessExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let object = Self::build_expr(&children[0])?;
        let member = token_if_kind(&children[1], "IDENT")
            .ok_or_else(|| AstError::MissingChild {
                node: "MemberAccessExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        Ok(Expr::MemberAccess {
            span,
            object: Box::new(object),
            member,
        })
    }

    fn build_method_call_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        if children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "MethodCallExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let object = Self::build_expr(&children[0])?;
        let method = token_if_kind(&children[1], "IDENT")
            .ok_or_else(|| AstError::MissingChild {
                node: "MethodCallExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let mut args = Vec::new();
        for arg in children.iter().skip(2) {
            args.push(Self::build_expr(arg)?);
        }

        Ok(Expr::MethodCall {
            span,
            object: Box::new(object),
            method,
            args,
        })
    }

    fn build_type_test_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        if children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "TypeTestExpr".to_string(),
                location: AstError::no_location(),
            });
        }
        let expr = Self::build_expr(&children[0])?;
        let type_name = token_if_kind(&children[1], "IDENT")
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeTestExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();
        Ok(Expr::TypeTest {
            span,
            expr: Box::new(expr),
            type_name,
        })
    }

    fn build_type_cast_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        if children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "TypeCastExpr".to_string(),
                location: AstError::no_location(),
            });
        }
        let expr = Self::build_expr(&children[0])?;
        let type_name = token_if_kind(&children[1], "IDENT")
            .ok_or_else(|| AstError::MissingChild {
                node: "TypeCastExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();
        Ok(Expr::TypeCast {
            span,
            expr: Box::new(expr),
            type_name,
        })
    }

    fn build_vector_literal_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        let mut elements = Vec::new();
        for child in children {
            elements.push(Self::build_expr(child)?);
        }
        Ok(Expr::VectorLiteral(elements))
    }

    fn build_vector_generator_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        // children: [body_expr, IDENT_token, iterable_expr]
        if children.len() < 3 {
            return Err(AstError::MissingChild {
                node: "VectorGeneratorExpr".to_string(),
                location: AstError::no_location(),
            });
        }
        let body = Self::build_expr(&children[0])?;
        let var = token_if_kind(&children[1], "IDENT")
            .ok_or_else(|| AstError::MissingChild {
                node: "VectorGeneratorExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();
        let iterable = Self::build_expr(&children[2])?;
        Ok(Expr::VectorGenerator {
            span,
            body: Box::new(body),
            var,
            iterable: Box::new(iterable),
        })
    }

    fn build_vector_index_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        if children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "VectorIndexExpr".to_string(),
                location: AstError::no_location(),
            });
        }
        let vector = Self::build_expr(&children[0])?;
        let index = Self::build_expr(&children[1])?;
        Ok(Expr::VectorIndex {
            span,
            vector: Box::new(vector),
            index: Box::new(index),
        })
    }

    fn build_lambda_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;

        let mut params = Vec::new();
        let mut return_type = None;

        // Collect all children except the last (which is the body).
        // Params are LambdaParam nodes; return type is LambdaReturnType node.
        for child in children.iter().take(children.len().saturating_sub(1)) {
            match as_node_name(child) {
                Some("LambdaParam") => params.push(Self::build_lambda_param(child)?),
                Some("LambdaReturnType") => {
                    return_type = Self::build_lambda_return_type(child)?;
                }
                _ => {}
            }
        }

        let body_node = children.last().ok_or_else(|| AstError::MissingChild {
            node: "LambdaExpr".to_string(),
            location: AstError::no_location(),
        })?;
        let body = Self::build_expr(body_node)?;
        let span = first_token_span(cst);

        Ok(Expr::Lambda {
            span,
            params,
            return_type,
            body: Box::new(body),
        })
    }

    fn build_lambda_param(cst: &CstNode) -> Result<Param, AstError> {
        let (_, children) = as_node(cst)?;
        let name = children
            .iter()
            .find_map(|n| token_if_kind(n, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "LambdaParam".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        // Second child (if any) is a type node from the Pratt parser
        let ty = if children.len() > 1 {
            Some(Self::build_typeref_from_pratt_node(&children[1])?)
        } else {
            None
        };

        Ok(Param { name, ty })
    }

    fn build_lambda_return_type(cst: &CstNode) -> Result<Option<TypeRef>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self::build_typeref_from_pratt_node(&children[0])?))
    }

    fn build_if_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        let expr_children = children
            .iter()
            .filter(|n| matches!(as_node_name(n), Some("Expr")))
            .collect::<Vec<_>>();

        if expr_children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "IfExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let mut branches = Vec::<(Expr, Expr)>::new();
        let first_cond = Self::build_expr(expr_children[0])?;
        let first_body = Self::build_expr(expr_children[1])?;
        branches.push((first_cond, first_body));

        if let Some(elif_list) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("ElifList")))
        {
            Self::collect_elif_list(elif_list, &mut branches)?;
        }

        let else_expr_node = expr_children.last().ok_or_else(|| AstError::MissingChild {
            node: "IfExpr".to_string(),
            location: AstError::no_location(),
        })?;
        let else_branch = Self::build_expr(else_expr_node)?;

        let span = first_token_span(cst);
        Ok(Expr::If {
            span,
            branches,
            else_branch: Box::new(else_branch),
        })
    }

    fn collect_elif_list(cst: &CstNode, out: &mut Vec<(Expr, Expr)>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        let expr_children = children
            .iter()
            .filter(|n| matches!(as_node_name(n), Some("Expr")))
            .collect::<Vec<_>>();

        if expr_children.len() >= 2 {
            let cond = Self::build_expr(expr_children[0])?;
            let body = Self::build_expr(expr_children[1])?;
            out.push((cond, body));
        }

        if let Some(next) = children
            .iter()
            .find(|n| matches!(as_node_name(n), Some("ElifList")))
        {
            Self::collect_elif_list(next, out)?;
        }

        Ok(())
    }

    fn build_while_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        let expr_children = children
            .iter()
            .filter(|n| matches!(as_node_name(n), Some("Expr")))
            .collect::<Vec<_>>();

        if expr_children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "WhileExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let condition = Self::build_expr(expr_children[0])?;
        let body = Self::build_expr(expr_children[1])?;

        let span = first_token_span(cst);
        Ok(Expr::While {
            span,
            condition: Box::new(condition),
            body: Box::new(body),
        })
    }

    fn build_for_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        // ForExpr -> FOR LPAREN IDENT IN Expr RPAREN Expr
        // children: [FOR_tok, LPAREN_tok, IDENT_tok, IN_tok, Expr_node, RPAREN_tok, Expr_node]

        let var = children
            .iter()
            .find_map(|n| token_if_kind(n, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "ForExpr".to_string(),
                location: AstError::no_location(),
            })?
            .to_string();

        let expr_nodes: Vec<_> = children
            .iter()
            .filter(|n| matches!(as_node_name(n), Some("Expr")))
            .collect();

        if expr_nodes.len() < 2 {
            return Err(AstError::MissingChild {
                node: "ForExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let iterable = Self::build_expr(expr_nodes[0])?;
        let body = Self::build_expr(expr_nodes[1])?;

        let span = first_token_span(cst);
        Ok(Expr::For {
            span,
            var,
            iterable: Box::new(iterable),
            body: Box::new(body),
        })
    }

    fn build_block_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        let Some(expr_list_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ExprList")))
        else {
            return Ok(Expr::Block(Vec::new()));
        };

        let mut exprs = Vec::new();
        Self::collect_expr_list(expr_list_node, &mut exprs)?;
        Ok(Expr::Block(exprs))
    }

    fn collect_expr_list(cst: &CstNode, out: &mut Vec<Expr>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(expr_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
        {
            out.push(Self::build_expr(expr_node)?);
        }

        if let Some(tail_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ExprListTail")))
        {
            Self::collect_expr_list_tail(tail_node, out)?;
        }

        Ok(())
    }

    fn collect_expr_list_tail(cst: &CstNode, out: &mut Vec<Expr>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(after_semi) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ExprListTailAfterSemi")))
        {
            Self::collect_expr_list_tail_after_semi(after_semi, out)?;
        }

        Ok(())
    }

    fn collect_expr_list_tail_after_semi(
        cst: &CstNode,
        out: &mut Vec<Expr>,
    ) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(expr_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
        {
            out.push(Self::build_expr(expr_node)?);
        }

        if let Some(next_tail) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ExprListTail")))
        {
            Self::collect_expr_list_tail(next_tail, out)?;
        }

        Ok(())
    }

    fn build_arg_list(cst: &CstNode) -> Result<Vec<Expr>, AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(Vec::new());
        }

        let mut args = Vec::new();
        if let Some(expr_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
        {
            args.push(Self::build_expr(expr_node)?);
        }

        if let Some(tail) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ArgListTail")))
        {
            Self::collect_arg_list_tail(tail, &mut args)?;
        }

        Ok(args)
    }

    fn collect_arg_list_tail(cst: &CstNode, out: &mut Vec<Expr>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        if let Some(expr_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
        {
            out.push(Self::build_expr(expr_node)?);
        }

        if let Some(next) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("ArgListTail")))
        {
            Self::collect_arg_list_tail(next, out)?;
        }

        Ok(())
    }

    fn build_let_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        let binding_node = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("LetBinding")))
            .ok_or_else(|| AstError::MissingChild {
                node: "LetExpr".to_string(),
                location: AstError::no_location(),
            })?;

        let mut bindings = vec![Self::build_let_binding(binding_node)?];

        if let Some(tail_node) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("LetBindingTail")))
        {
            Self::collect_let_binding_tail(tail_node, &mut bindings)?;
        }

        let body_expr_node = children
            .iter()
            .rev()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
            .ok_or_else(|| AstError::MissingChild {
                node: "LetExpr".to_string(),
                location: AstError::no_location(),
            })?;

        let body = Self::build_expr(body_expr_node)?;

        let span = first_token_span(cst);
        Ok(Expr::Let {
            span,
            bindings,
            body: Box::new(body),
        })
    }

    fn collect_let_binding_tail(cst: &CstNode, out: &mut Vec<LetBinding>) -> Result<(), AstError> {
        let (_, children) = as_node(cst)?;
        if children.is_empty() {
            return Ok(());
        }

        let binding_node = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("LetBinding")))
            .ok_or_else(|| AstError::MissingChild {
                node: "LetBindingTail".to_string(),
                location: AstError::no_location(),
            })?;

        out.push(Self::build_let_binding(binding_node)?);

        if let Some(next_tail) = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("LetBindingTail")))
        {
            Self::collect_let_binding_tail(next_tail, out)?;
        }

        Ok(())
    }

    fn build_let_binding(cst: &CstNode) -> Result<LetBinding, AstError> {
        let (_, children) = as_node(cst)?;

        let ident = children
            .iter()
            .find_map(|node| token_if_kind(node, "IDENT"))
            .ok_or_else(|| AstError::MissingChild {
                node: "LetBinding".to_string(),
                location: AstError::no_location(),
            })?;

        // Optional type annotation
        let ty = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("TypeAnnotation")))
            .map(Self::build_optional_typeref)
            .transpose()?
            .unwrap_or(None);

        let expr_node = children
            .iter()
            .find(|node| matches!(as_node_name(node), Some("Expr")))
            .ok_or_else(|| AstError::MissingChild {
                node: "LetBinding".to_string(),
                location: AstError::no_location(),
            })?;

        let value = Self::build_expr(expr_node)?;
        Ok(LetBinding {
            name: ident.to_string(),
            ty,
            value,
        })
    }

    fn build_binary_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        if children.len() < 3 {
            return Err(AstError::MissingChild {
                node: "BinaryExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let (op_kind, line, column) = as_token_meta(&children[0])?;
        let span = Span::new(line, column);
        let left = Self::build_expr(&children[1])?;
        let right = Self::build_expr(&children[2])?;

        if op_kind == "ASSIGN" {
            return Ok(Expr::Assign {
                span,
                target: Box::new(left),
                value: Box::new(right),
            });
        }

        let op = match op_kind {
            "PLUS" => BinaryOp::Add,
            "MINUS" => BinaryOp::Sub,
            "STAR" => BinaryOp::Mul,
            "SLASH" => BinaryOp::Div,
            "MOD" => BinaryOp::Mod,
            "POW" => BinaryOp::Pow,
            "AT" => BinaryOp::Concat,
            "ATAT" => BinaryOp::ConcatSpace,
            "EQ" => BinaryOp::Eq,
            "NEQ" => BinaryOp::Neq,
            "LT" => BinaryOp::Lt,
            "LE" => BinaryOp::Le,
            "GT" => BinaryOp::Gt,
            "GE" => BinaryOp::Ge,
            "AND" => BinaryOp::And,
            "OR" => BinaryOp::Or,
            other => {
                return Err(AstError::UnexpectedToken {
                    kind: other.to_string(),
                    location: AstError::at(line, column),
                });
            }
        };

        Ok(Expr::Binary {
            span,
            left: Box::new(left),
            op,
            right: Box::new(right),
        })
    }

    fn build_unary_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let (_, children) = as_node(cst)?;
        if children.len() < 2 {
            return Err(AstError::MissingChild {
                node: "UnaryExpr".to_string(),
                location: AstError::no_location(),
            });
        }

        let (op_kind, line, column) = as_token_meta(&children[0])?;
        let span = Span::new(line, column);
        let expr = Self::build_expr(&children[1])?;

        let op = match op_kind {
            "NOT" => UnaryOp::Not,
            "MINUS" => UnaryOp::Neg,
            "PLUS" => UnaryOp::Pos,
            other => {
                return Err(AstError::UnexpectedToken {
                    kind: other.to_string(),
                    location: AstError::at(line, column),
                });
            }
        };

        Ok(Expr::Unary {
            span,
            op,
            expr: Box::new(expr),
        })
    }

    fn build_call_expr(cst: &CstNode) -> Result<Expr, AstError> {
        let span = first_token_span(cst);
        let (_, children) = as_node(cst)?;
        let callee_node = children.first().ok_or_else(|| AstError::MissingChild {
            node: "CallExpr".to_string(),
            location: AstError::no_location(),
        })?;

        let callee = Self::build_expr(callee_node)?;
        let mut args = Vec::new();
        for arg in children.iter().skip(1) {
            args.push(Self::build_expr(arg)?);
        }

        Ok(Expr::Call {
            span,
            callee: Box::new(callee),
            args,
        })
    }

    fn build_primary_token(kind: &str, lexeme: &str, node: &CstNode) -> Result<Expr, AstError> {
        match kind {
            "NUMBER" => {
                let value = lexeme
                    .parse::<f64>()
                    .map_err(|_| AstError::InvalidNumberLiteral {
                        literal: lexeme.to_string(),
                        location: token_location(node),
                    })?;
                Ok(Expr::Number(value))
            }
            "STRING" => Ok(Expr::String(lexeme.to_string())),
            "TRUE" => Ok(Expr::Bool(true)),
            "FALSE" => Ok(Expr::Bool(false)),
            "IDENT" => {
                let span = match node {
                    CstNode::Token { line, column, .. } => Span::new(*line, *column),
                    _ => Span::default(),
                };
                Ok(Expr::Var(lexeme.to_string(), span))
            }
            other => Err(AstError::UnexpectedToken {
                kind: other.to_string(),
                location: token_location(node),
            }),
        }
    }
}

fn as_node(cst: &CstNode) -> Result<(&str, &[CstNode]), AstError> {
    match cst {
        CstNode::Node { name, children } => Ok((name.as_str(), children.as_slice())),
        CstNode::Token {
            kind, line, column, ..
        } => Err(AstError::UnexpectedToken {
            kind: kind.clone(),
            location: AstError::at(*line, *column),
        }),
        CstNode::Error {
            message,
            line,
            column,
        } => Err(AstError::UnsupportedConstruct {
            message: message.clone(),
            location: AstError::at(*line, *column),
        }),
    }
}

fn as_node_name(cst: &CstNode) -> Option<&str> {
    if let CstNode::Node { name, .. } = cst {
        Some(name.as_str())
    } else {
        None
    }
}

fn as_token_kind(cst: &CstNode) -> Option<&str> {
    if let CstNode::Token { kind, .. } = cst {
        Some(kind.as_str())
    } else {
        None
    }
}

fn token_if_kind<'a>(cst: &'a CstNode, expected_kind: &str) -> Option<&'a str> {
    if let CstNode::Token { kind, lexeme, .. } = cst
        && kind == expected_kind
    {
        return Some(lexeme.as_str());
    }
    None
}

fn token_if_kind_with_span<'a>(cst: &'a CstNode, expected_kind: &str) -> Option<(&'a str, Span)> {
    if let CstNode::Token {
        kind,
        lexeme,
        line,
        column,
        ..
    } = cst
        && kind == expected_kind
    {
        return Some((lexeme.as_str(), Span::new(*line, *column)));
    }
    None
}

fn as_token_meta(cst: &CstNode) -> Result<(&str, usize, usize), AstError> {
    match cst {
        CstNode::Token {
            kind, line, column, ..
        } => Ok((kind.as_str(), *line, *column)),
        CstNode::Node { name, .. } => Err(AstError::UnexpectedNode {
            name: name.clone(),
            location: AstError::no_location(),
        }),
        CstNode::Error {
            message,
            line,
            column,
        } => Err(AstError::UnsupportedConstruct {
            message: message.clone(),
            location: AstError::at(*line, *column),
        }),
    }
}

fn token_location(cst: &CstNode) -> String {
    match cst {
        CstNode::Token { line, column, .. } => AstError::at(*line, *column),
        _ => AstError::no_location(),
    }
}

/// Walk to the first token in a CST sub-tree and return its span.
fn first_token_span(cst: &CstNode) -> Span {
    match cst {
        CstNode::Token { line, column, .. } => Span::new(*line, *column),
        CstNode::Node { children, .. } => {
            children.first().map(first_token_span).unwrap_or_default()
        }
        CstNode::Error { line, column, .. } => Span::new(*line, *column),
    }
}
