use crate::error::SemanticError;
use crate::types::Type;
use hulk_frontend::ast::{Decl, Program, TypeMember};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub constructor_params: Vec<(String, Type)>,
    pub parent: Option<String>,
    pub attributes: HashMap<String, Type>,
    pub methods: HashMap<String, MethodInfo>,
}

#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub name: String,
    pub parent: Option<String>,
    pub methods: HashMap<String, MethodInfo>,
}

#[derive(Debug, Clone)]
pub struct TypeRegistry {
    types: HashMap<String, TypeInfo>,
    protocols: HashMap<String, ProtocolInfo>,
}

impl TypeRegistry {
    /// Build the registry from a parsed program (Pass 0 of semantic checking).
    pub fn build(program: &Program) -> Result<Self, SemanticError> {
        let mut registry = Self {
            types: HashMap::new(),
            protocols: HashMap::new(),
        };

        // Pass A: collect all names so members can forward-reference each other.
        for decl in &program.declarations {
            match decl {
                Decl::Type(td) => {
                    if registry.types.contains_key(&td.name)
                        || registry.protocols.contains_key(&td.name)
                    {
                        return Err(SemanticError::DuplicateType {
                            name: td.name.clone(),
                        });
                    }
                    registry.types.insert(
                        td.name.clone(),
                        TypeInfo {
                            name: td.name.clone(),
                            constructor_params: vec![],
                            parent: None,
                            attributes: HashMap::new(),
                            methods: HashMap::new(),
                        },
                    );
                }
                Decl::Protocol(pd) => {
                    if registry.protocols.contains_key(&pd.name)
                        || registry.types.contains_key(&pd.name)
                    {
                        return Err(SemanticError::DuplicateType {
                            name: pd.name.clone(),
                        });
                    }
                    registry.protocols.insert(
                        pd.name.clone(),
                        ProtocolInfo {
                            name: pd.name.clone(),
                            parent: None,
                            methods: HashMap::new(),
                        },
                    );
                }
                Decl::Function(_) => {}
            }
        }

        // Pass B: fill in full type details.
        for decl in &program.declarations {
            match decl {
                Decl::Type(td) => {
                    let constructor_params = td
                        .params
                        .iter()
                        .map(|p| {
                            let ty =
                                p.ty.as_ref()
                                    .map(Type::from_type_ref)
                                    .unwrap_or(Type::Unknown);
                            (p.name.clone(), ty)
                        })
                        .collect();

                    let parent = td.parent.as_ref().map(|p| p.name.clone());

                    let mut attributes = HashMap::new();
                    let mut methods = HashMap::new();

                    for member in &td.members {
                        match member {
                            TypeMember::Attribute(attr) => {
                                if attributes.contains_key(&attr.name)
                                    || methods.contains_key(&attr.name)
                                {
                                    return Err(SemanticError::DuplicateAttribute {
                                        type_name: td.name.clone(),
                                        attr_name: attr.name.clone(),
                                    });
                                }
                                let ty = attr
                                    .ty
                                    .as_ref()
                                    .map(Type::from_type_ref)
                                    .unwrap_or(Type::Unknown);
                                attributes.insert(attr.name.clone(), ty);
                            }
                            TypeMember::Method(method) => {
                                if methods.contains_key(&method.name)
                                    || attributes.contains_key(&method.name)
                                {
                                    return Err(SemanticError::DuplicateMethod {
                                        type_name: td.name.clone(),
                                        method_name: method.name.clone(),
                                    });
                                }
                                let params = method
                                    .params
                                    .iter()
                                    .map(|p| {
                                        p.ty.as_ref()
                                            .map(Type::from_type_ref)
                                            .unwrap_or(Type::Unknown)
                                    })
                                    .collect();
                                let return_type = method
                                    .return_type
                                    .as_ref()
                                    .map(Type::from_type_ref)
                                    .unwrap_or(Type::Unknown);
                                methods.insert(
                                    method.name.clone(),
                                    MethodInfo {
                                        params,
                                        return_type,
                                    },
                                );
                            }
                        }
                    }

                    registry.types.insert(
                        td.name.clone(),
                        TypeInfo {
                            name: td.name.clone(),
                            constructor_params,
                            parent,
                            attributes,
                            methods,
                        },
                    );
                }
                Decl::Protocol(pd) => {
                    let mut methods = HashMap::new();
                    for method in &pd.methods {
                        if methods.contains_key(&method.name) {
                            return Err(SemanticError::DuplicateProtocolMethod {
                                protocol_name: pd.name.clone(),
                                method_name: method.name.clone(),
                            });
                        }
                        let params = method
                            .params
                            .iter()
                            .map(|p| {
                                p.ty.as_ref()
                                    .map(Type::from_type_ref)
                                    .unwrap_or(Type::Unknown)
                            })
                            .collect();
                        let return_type = method
                            .return_type
                            .as_ref()
                            .map(Type::from_type_ref)
                            .unwrap_or(Type::Unknown);
                        methods.insert(
                            method.name.clone(),
                            MethodInfo {
                                params,
                                return_type,
                            },
                        );
                    }
                    registry.protocols.insert(
                        pd.name.clone(),
                        ProtocolInfo {
                            name: pd.name.clone(),
                            parent: pd.parent.clone(),
                            methods,
                        },
                    );
                }
                Decl::Function(_) => {}
            }
        }

        // Pass C: propagate constructor params for passthrough inheritance
        // (when a type has no explicit params AND no `inherits Parent(args)` clause).
        let type_names: Vec<String> = registry.types.keys().cloned().collect();
        for name in &type_names {
            let (has_own_params, parent_name, is_passthrough) = {
                let ti = registry.types.get(name).unwrap();
                let parent_name = ti.parent.clone();
                // passthrough = declared with no own params, and parent has no explicit arg list
                let td_opt = program.declarations.iter().find_map(|d| {
                    if let Decl::Type(td) = d {
                        if td.name == *name {
                            return Some(td);
                        }
                    }
                    None
                });
                let is_passthrough = td_opt
                    .map(|td| {
                        td.params.is_empty()
                            && td
                                .parent
                                .as_ref()
                                .map(|p| p.args.is_none())
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                (ti.constructor_params.len(), parent_name, is_passthrough)
            };
            if has_own_params == 0 && is_passthrough {
                if let Some(pname) = parent_name {
                    // Collect parent params (which may themselves have been filled in already).
                    let parent_params = registry
                        .types
                        .get(&pname)
                        .map(|p| p.constructor_params.clone())
                        .unwrap_or_default();
                    if !parent_params.is_empty() {
                        registry.types.get_mut(name).unwrap().constructor_params = parent_params;
                    }
                }
            }
        }

        // W2f: Pre-register builtin Iterable protocol (if not user-defined).
        registry
            .protocols
            .entry("Iterable".to_string())
            .or_insert_with(|| {
                let mut methods = HashMap::new();
                methods.insert(
                    "next".to_string(),
                    MethodInfo {
                        params: vec![],
                        return_type: Type::Boolean,
                    },
                );
                methods.insert(
                    "current".to_string(),
                    MethodInfo {
                        params: vec![],
                        return_type: Type::Object,
                    },
                );
                ProtocolInfo {
                    name: "Iterable".to_string(),
                    parent: None,
                    methods,
                }
            });

        registry.validate_inheritance()?;
        registry.validate_protocol_conformance()?;
        Ok(registry)
    }

    pub fn type_exists(&self, name: &str) -> bool {
        self.types.contains_key(name) || self.protocols.contains_key(name)
    }

    pub fn get_type(&self, name: &str) -> Option<&TypeInfo> {
        self.types.get(name)
    }

    pub fn get_protocol(&self, name: &str) -> Option<&ProtocolInfo> {
        self.protocols.get(name)
    }

    /// Returns an error if `name` is not a known type or protocol.
    pub fn validate_user_type(&self, name: &str) -> Result<(), SemanticError> {
        if self.type_exists(name) {
            Ok(())
        } else {
            Err(SemanticError::UndefinedType {
                name: name.to_string(),
            })
        }
    }

    /// Returns true if `sub` is the same as `ancestor`, or inherits from it directly or transitively.
    /// Returns true if `type_name` structurally implements Iterable(elem_type):
    /// it must have `next(): Boolean` and `current(): elem_type`.
    pub fn implements_iterable(&self, type_name: &str, elem_type: &Type) -> bool {
        let Some(next_info) = self.lookup_method_info(type_name, "next") else {
            return false;
        };
        if next_info.return_type != Type::Boolean && next_info.return_type != Type::Unknown {
            return false;
        }
        let Some(current_info) = self.lookup_method_info(type_name, "current") else {
            return false;
        };
        // The current element type must be compatible with elem_type.
        current_info.return_type == *elem_type
            || current_info.return_type == Type::Unknown
            || *elem_type == Type::Unknown
    }

    pub fn is_descendant_of(&self, sub: &str, ancestor: &str) -> bool {
        let mut current = Some(sub);
        while let Some(name) = current {
            if name == ancestor {
                return true;
            }
            current = self.types.get(name).and_then(|ti| ti.parent.as_deref());
        }
        false
    }

    /// Finds an attribute by traversing the inheritance chain upwards. Returns a cloned Type.
    pub fn lookup_attribute(&self, type_name: &str, attr_name: &str) -> Option<Type> {
        let mut current = Some(type_name);
        while let Some(name) = current {
            let Some(ti) = self.types.get(name) else {
                break;
            };
            if let Some(ty) = ti.attributes.get(attr_name) {
                return Some(ty.clone());
            }
            current = ti.parent.as_deref();
        }
        None
    }

    /// Update an attribute's type after its initializer is analyzed (for unannotated attrs).
    pub fn update_attribute_type(
        &mut self,
        type_name: &str,
        attr_name: &str,
        attr_type: Type,
    ) {
        if let Some(ti) = self.types.get_mut(type_name) {
            if let Some(slot) = ti.attributes.get_mut(attr_name) {
                if *slot == Type::Unknown {
                    *slot = attr_type;
                }
            }
        }
    }

    /// Update a method's return type after the body is analyzed (for unannotated methods).
    pub fn update_method_return_type(
        &mut self,
        type_name: &str,
        method_name: &str,
        return_type: Type,
    ) {
        if let Some(ti) = self.types.get_mut(type_name) {
            if let Some(mi) = ti.methods.get_mut(method_name) {
                if mi.return_type == Type::Unknown {
                    mi.return_type = return_type;
                }
            }
        }
    }

    /// Finds a method by traversing the inheritance chain upwards. Returns a cloned MethodInfo.
    /// W2d: If type_name is itself a protocol (not a concrete type), also checks protocols.
    pub fn lookup_method_info(&self, type_name: &str, method_name: &str) -> Option<MethodInfo> {
        let mut current = Some(type_name);
        while let Some(name) = current {
            let Some(ti) = self.types.get(name) else {
                break;
            };
            if let Some(mi) = ti.methods.get(method_name) {
                return Some(mi.clone());
            }
            current = ti.parent.as_deref();
        }
        // W2d: Fall back to protocol lookup (enables method resolution on protocol-typed vars).
        if let Some(proto) = self.protocols.get(type_name) {
            if let Some(mi) = proto.methods.get(method_name) {
                return Some(mi.clone());
            }
        }
        None
    }

    /// Finds a method and returns the concrete owner type that defines it.
    pub fn lookup_method_owner_info(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<(String, MethodInfo)> {
        let mut current = Some(type_name);
        while let Some(name) = current {
            let Some(ti) = self.types.get(name) else {
                break;
            };
            if let Some(mi) = ti.methods.get(method_name) {
                return Some((name.to_string(), mi.clone()));
            }
            current = ti.parent.as_deref();
        }
        if let Some(proto) = self.protocols.get(type_name) {
            if let Some(mi) = proto.methods.get(method_name) {
                return Some((type_name.to_string(), mi.clone()));
            }
        }
        None
    }

    /// Resolves `base(args)` from a method body.
    ///
    /// The regular rule is to call the parent's implementation of the current method.
    /// As a compatibility fallback for wrapper methods, if the parent has no such method,
    /// choose a single parent/ancestor method with matching arity and return type.
    pub fn resolve_base_method_info(
        &self,
        current_type: &str,
        parent_type: &str,
        current_method: &str,
        arg_count: usize,
    ) -> Option<(String, MethodInfo)> {
        if let Some(info) = self.lookup_method_info(parent_type, current_method) {
            return Some((current_method.to_string(), info));
        }

        let current_info = self.lookup_method_info(current_type, current_method)?;
        let mut matches = Vec::new();
        let mut current = Some(parent_type);
        let mut seen = HashSet::new();
        while let Some(name) = current {
            let Some(ti) = self.types.get(name) else {
                break;
            };
            for (method_name, info) in &ti.methods {
                if seen.insert(method_name.clone())
                    && info.params.len() == arg_count
                    && info.return_type == current_info.return_type
                {
                    matches.push((method_name.clone(), info.clone()));
                }
            }
            current = ti.parent.as_deref();
        }

        if matches.len() == 1 {
            matches.pop()
        } else {
            None
        }
    }

    /// Returns the nearest common ancestor of two type names in the hierarchy, or Object.
    pub fn least_common_ancestor(&self, a: &str, b: &str) -> crate::types::Type {
        // Collect the ancestor chain of a (inclusive).
        let mut a_chain: Vec<&str> = Vec::new();
        let mut cur = Some(a);
        while let Some(name) = cur {
            a_chain.push(name);
            cur = self.types.get(name).and_then(|ti| ti.parent.as_deref());
        }

        // Walk b's chain until we hit a name that is also in a's chain.
        let mut cur_b = Some(b);
        while let Some(name) = cur_b {
            if a_chain.contains(&name) {
                return crate::types::Type::UserType(name.to_string());
            }
            cur_b = self.types.get(name).and_then(|ti| ti.parent.as_deref());
        }

        crate::types::Type::Object
    }

    fn validate_inheritance(&self) -> Result<(), SemanticError> {
        // Check that every declared parent exists, no primitives inherited, no cycles.
        for (name, info) in &self.types {
            if let Some(parent_name) = &info.parent {
                // W2a: Check primitives first (they don't appear in type_exists).
                if matches!(parent_name.as_str(), "Number" | "String" | "Boolean") {
                    return Err(SemanticError::CannotInheritFromPrimitive {
                        child: name.clone(),
                        parent: parent_name.clone(),
                    });
                }
                if !self.type_exists(parent_name) {
                    return Err(SemanticError::UndefinedType {
                        name: parent_name.clone(),
                    });
                }
            }

            let mut visited: HashSet<&str> = HashSet::new();
            let mut current: Option<&str> = Some(name.as_str());
            while let Some(cur) = current {
                if !visited.insert(cur) {
                    return Err(SemanticError::CircularInheritance { name: name.clone() });
                }
                current = self.types.get(cur).and_then(|ti| ti.parent.as_deref());
            }

            // W2c: Method override signatures must match the parent exactly.
            self.validate_method_overrides(name)?;
        }

        for info in self.protocols.values() {
            if let Some(parent_name) = &info.parent {
                if !self.protocols.contains_key(parent_name) {
                    return Err(SemanticError::UndefinedType {
                        name: parent_name.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// W2c: For each method the type defines that also exists in its concrete parent,
    /// verify the signatures match exactly (same arity, param types, return type).
    fn validate_method_overrides(&self, type_name: &str) -> Result<(), SemanticError> {
        let Some(type_info) = self.types.get(type_name) else {
            return Ok(());
        };
        let Some(parent_name) = type_info.parent.as_deref() else {
            return Ok(());
        };
        // Only validate against concrete parents; protocol conformance is checked separately.
        if !self.types.contains_key(parent_name) {
            return Ok(());
        }
        let own_methods: Vec<(String, MethodInfo)> = type_info
            .methods
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (method_name, own_sig) in &own_methods {
            if let Some(parent_sig) = self.lookup_method_info(parent_name, &method_name) {
                let ok = own_sig.params.len() == parent_sig.params.len()
                    && own_sig.params == parent_sig.params
                    && own_sig.return_type == parent_sig.return_type;
                if !ok {
                    return Err(SemanticError::MethodOverrideSignatureMismatch {
                        type_name: type_name.to_string(),
                        method_name: method_name.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// For every type whose declared parent is a protocol, verify structural conformance.
    fn validate_protocol_conformance(&self) -> Result<(), SemanticError> {
        for (type_name, type_info) in &self.types {
            if let Some(parent_name) = &type_info.parent {
                if let Some(protocol) = self.protocols.get(parent_name) {
                    self.check_type_vs_protocol(type_name, protocol)?;
                    // Also satisfy the protocol's own parent (if it extends another protocol).
                    if let Some(grand_name) = &protocol.parent {
                        if let Some(grand_proto) = self.protocols.get(grand_name.as_str()) {
                            self.check_type_vs_protocol(type_name, grand_proto)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// W2b: Full protocol signature check (arity + covariant return + contravariant params).
    fn check_type_vs_protocol(
        &self,
        type_name: &str,
        protocol: &ProtocolInfo,
    ) -> Result<(), SemanticError> {
        let methods: Vec<(String, MethodInfo)> = protocol
            .methods
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (method_name, expected_sig) in &methods {
            let actual_sig = match self.lookup_method_info(type_name, method_name) {
                None => {
                    return Err(SemanticError::MissingProtocolMethod {
                        type_name: type_name.to_string(),
                        method_name: method_name.clone(),
                    });
                }
                Some(s) => s,
            };
            if actual_sig.params.len() != expected_sig.params.len() {
                return Err(SemanticError::ProtocolMethodSignatureMismatch {
                    type_name: type_name.to_string(),
                    method_name: method_name.clone(),
                    expected: expected_sig.params.len(),
                    found: actual_sig.params.len(),
                });
            }
            // Covariant return: actual_ret must be assignable to expected_ret.
            if !self.is_type_assignable(&actual_sig.return_type, &expected_sig.return_type) {
                return Err(SemanticError::ProtocolReturnTypeMismatch {
                    type_name: type_name.to_string(),
                    method_name: method_name.clone(),
                    expected: expected_sig.return_type.clone(),
                    found: actual_sig.return_type.clone(),
                });
            }
            // Contravariant params: expected_param must be assignable to actual_param.
            for (idx, (actual_p, expected_p)) in actual_sig
                .params
                .iter()
                .zip(expected_sig.params.iter())
                .enumerate()
            {
                if !self.is_type_assignable(expected_p, actual_p) {
                    return Err(SemanticError::ProtocolParamTypeMismatch {
                        type_name: type_name.to_string(),
                        method_name: method_name.clone(),
                        param_idx: idx,
                        expected: expected_p.clone(),
                        found: actual_p.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// W2e: Returns true when `type_name` structurally satisfies `protocol_name`.
    pub fn implicitly_conforms_to_protocol(&self, type_name: &str, protocol_name: &str) -> bool {
        let Some(protocol) = self.protocols.get(protocol_name) else {
            return false;
        };
        let methods: Vec<(String, MethodInfo)> = protocol
            .methods
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (method_name, expected_sig) in &methods {
            let Some(actual_sig) = self.lookup_method_info(type_name, method_name) else {
                return false;
            };
            if actual_sig.params.len() != expected_sig.params.len() {
                return false;
            }
            if !self.is_type_assignable(&actual_sig.return_type, &expected_sig.return_type) {
                return false;
            }
            for (actual_p, expected_p) in actual_sig.params.iter().zip(expected_sig.params.iter()) {
                if !self.is_type_assignable(expected_p, actual_p) {
                    return false;
                }
            }
        }
        true
    }

    /// Registry-aware assignability, mirroring the checker's `is_assignable`.
    fn is_type_assignable(&self, sub: &Type, target: &Type) -> bool {
        if *sub == Type::Unknown || *target == Type::Unknown {
            return true;
        }
        if sub == target || *target == Type::Object {
            return true;
        }
        match (sub, target) {
            (Type::UserType(sn), Type::UserType(tn)) => self.is_descendant_of(sn, tn),
            (Type::UserType(_), Type::Object) => true,
            (Type::Vector(si), Type::Vector(ti)) => self.is_type_assignable(si, ti),
            (Type::Iterable(si), Type::Iterable(ti)) => self.is_type_assignable(si, ti),
            _ => false,
        }
    }
}
