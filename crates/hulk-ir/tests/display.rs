use hulk_ir::{
    AttrId, DataId, FunctionId, IrAttribute, IrBinaryOp, IrData, IrDataValue, IrFunction,
    IrFunctionKind, IrInstr, IrLocal, IrMethod, IrParam, IrPlace, IrProgram, IrTemp, IrType,
    IrTypeRef, IrUnaryOp, IrValue, LabelId, LocalId, MethodSlot, ParamId, TempId, TypeId,
};

#[test]
fn displays_minimal_arithmetic_program() {
    let program = IrProgram {
        types: vec![],
        data: vec![],
        entry: FunctionId(0),
        functions: vec![IrFunction {
            id: FunctionId(0),
            name: "entry".to_string(),
            kind: IrFunctionKind::Entry,
            params: vec![],
            locals: vec![IrLocal {
                id: LocalId(0),
                name: "x".to_string(),
                ty: IrTypeRef::Number,
            }],
            temps: vec![IrTemp {
                id: TempId(0),
                ty: IrTypeRef::Number,
            }],
            return_type: IrTypeRef::Number,
            body: vec![
                IrInstr::Assign {
                    dst: IrPlace::Local(LocalId(0)),
                    src: IrValue::ConstNumber(1.0),
                },
                IrInstr::Binary {
                    dst: IrPlace::Temp(TempId(0)),
                    op: IrBinaryOp::Add,
                    left: IrValue::Local(LocalId(0)),
                    right: IrValue::ConstNumber(2.0),
                },
                IrInstr::Return(Some(IrValue::Temp(TempId(0)))),
            ],
        }],
    };

    assert_eq!(
        program.to_string(),
        r#".TYPES
  <empty>

.DATA
  <empty>

.CODE
entry #0
function entry #0 -> Number {
  local %l0: Number name=x
  temp %t0: Number

  %l0 = 1
  %t0 = %l0 + 2
  return %t0
}
"#
    );
}

#[test]
fn displays_object_layout_and_method_program() {
    let program = IrProgram {
        types: vec![IrType {
            id: TypeId(0),
            name: "Point".to_string(),
            parent: None,
            attributes: vec![
                IrAttribute {
                    id: AttrId(0),
                    name: "x".to_string(),
                    ty: IrTypeRef::Number,
                },
                IrAttribute {
                    id: AttrId(1),
                    name: "y".to_string(),
                    ty: IrTypeRef::Number,
                },
            ],
            methods: vec![IrMethod {
                slot: MethodSlot(0),
                name: "getX".to_string(),
                function: "Point_getX".to_string(),
            }],
        }],
        data: vec![IrData {
            id: DataId(0),
            value: IrDataValue::String("done".to_string()),
        }],
        entry: FunctionId(0),
        functions: vec![
            IrFunction {
                id: FunctionId(0),
                name: "entry".to_string(),
                kind: IrFunctionKind::Entry,
                params: vec![],
                locals: vec![IrLocal {
                    id: LocalId(0),
                    name: "p".to_string(),
                    ty: IrTypeRef::User("Point".to_string()),
                }],
                temps: vec![IrTemp {
                    id: TempId(0),
                    ty: IrTypeRef::Number,
                }],
                return_type: IrTypeRef::Object,
                body: vec![
                    IrInstr::Allocate {
                        dst: IrPlace::Local(LocalId(0)),
                        type_name: "Point".to_string(),
                    },
                    IrInstr::VirtualCall {
                        dst: Some(IrPlace::Temp(TempId(0))),
                        receiver: IrValue::Local(LocalId(0)),
                        receiver_static_type: "Point".to_string(),
                        method: "getX".to_string(),
                        slot: MethodSlot(0),
                        args: vec![],
                    },
                    IrInstr::Call {
                        dst: None,
                        function: "print".to_string(),
                        args: vec![IrValue::DataRef(DataId(0))],
                    },
                    IrInstr::Return(Some(IrValue::Temp(TempId(0)))),
                ],
            },
            IrFunction {
                id: FunctionId(1),
                name: "Point_getX".to_string(),
                kind: IrFunctionKind::Method {
                    owner_type: "Point".to_string(),
                    method_name: "getX".to_string(),
                },
                params: vec![IrParam {
                    id: ParamId(0),
                    name: "self".to_string(),
                    ty: IrTypeRef::User("Point".to_string()),
                }],
                locals: vec![],
                temps: vec![IrTemp {
                    id: TempId(0),
                    ty: IrTypeRef::Number,
                }],
                return_type: IrTypeRef::Number,
                body: vec![
                    IrInstr::GetAttr {
                        dst: IrPlace::Temp(TempId(0)),
                        object: IrValue::Param(ParamId(0)),
                        attr: AttrId(0),
                    },
                    IrInstr::Return(Some(IrValue::Temp(TempId(0)))),
                ],
            },
        ],
    };

    assert_eq!(
        program.to_string(),
        r#".TYPES
type Point #0 {
  attr #0 x: Number
  attr #1 y: Number
  method #0 getX : Point_getX
}

.DATA
data @s0 = "done"

.CODE
entry #0
function entry #0 -> Object {
  local %l0: Point name=p
  temp %t0: Number

  %l0 = allocate Point
  %t0 = vcall %l0.Point::getX#0()
  call print(@s0)
  return %t0
}
method Point.getX #1 -> Number {
  param %p0: Point name=self
  temp %t0: Number

  %t0 = getattr %p0, #0
  return %t0
}
"#
    );
}

#[test]
fn displays_control_flow_vectors_and_closures() {
    let program = IrProgram {
        types: vec![],
        data: vec![],
        entry: FunctionId(0),
        functions: vec![IrFunction {
            id: FunctionId(0),
            name: "entry".to_string(),
            kind: IrFunctionKind::Entry,
            params: vec![],
            locals: vec![IrLocal {
                id: LocalId(0),
                name: "items".to_string(),
                ty: IrTypeRef::Vector(Box::new(IrTypeRef::Number)),
            }],
            temps: vec![
                IrTemp {
                    id: TempId(0),
                    ty: IrTypeRef::Vector(Box::new(IrTypeRef::Number)),
                },
                IrTemp {
                    id: TempId(1),
                    ty: IrTypeRef::Number,
                },
                IrTemp {
                    id: TempId(2),
                    ty: IrTypeRef::Functor {
                        capture_types: vec![],
                        params: vec![IrTypeRef::Number],
                        ret: Box::new(IrTypeRef::Boolean),
                    },
                },
            ],
            return_type: IrTypeRef::Number,
            body: vec![
                IrInstr::NewVector {
                    dst: IrPlace::Temp(TempId(0)),
                    elements: vec![IrValue::ConstNumber(1.0), IrValue::ConstNumber(2.0)],
                },
                IrInstr::Assign {
                    dst: IrPlace::Local(LocalId(0)),
                    src: IrValue::Temp(TempId(0)),
                },
                IrInstr::Label(LabelId(0)),
                IrInstr::VectorLen {
                    dst: IrPlace::Temp(TempId(1)),
                    vector: IrValue::Local(LocalId(0)),
                },
                IrInstr::Branch {
                    cond: IrValue::Temp(TempId(1)),
                    then_label: LabelId(1),
                    else_label: LabelId(2),
                },
                IrInstr::Label(LabelId(1)),
                IrInstr::MakeClosure {
                    dst: IrPlace::Temp(TempId(2)),
                    function: "lambda_0".to_string(),
                    captures: vec![IrValue::Local(LocalId(0))],
                },
                IrInstr::Jump(LabelId(0)),
                IrInstr::Label(LabelId(2)),
                IrInstr::Return(Some(IrValue::Temp(TempId(1)))),
            ],
        }],
    };

    assert_eq!(
        program.to_string(),
        r#".TYPES
  <empty>

.DATA
  <empty>

.CODE
entry #0
function entry #0 -> Number {
  local %l0: Number[] name=items
  temp %t0: Number[]
  temp %t1: Number
  temp %t2: (Number) -> Boolean

  %t0 = vector [1, 2]
  %l0 = %t0
  label L0
  %t1 = vector_len %l0
  branch %t1 ? L1 : L2
  label L1
  %t2 = closure lambda_0 captures [%l0]
  jump L0
  label L2
  return %t1
}
"#
    );
}

#[test]
fn displays_remaining_instruction_forms() {
    let program = IrProgram {
        types: vec![],
        data: vec![
            IrData {
                id: DataId(0),
                value: IrDataValue::Number(3.5),
            },
            IrData {
                id: DataId(1),
                value: IrDataValue::Boolean(false),
            },
        ],
        entry: FunctionId(0),
        functions: vec![
            IrFunction {
                id: FunctionId(0),
                name: "entry".to_string(),
                kind: IrFunctionKind::Entry,
                params: vec![],
                locals: vec![
                    IrLocal {
                        id: LocalId(0),
                        name: "obj".to_string(),
                        ty: IrTypeRef::User("Box".to_string()),
                    },
                    IrLocal {
                        id: LocalId(1),
                        name: "items".to_string(),
                        ty: IrTypeRef::Vector(Box::new(IrTypeRef::Number)),
                    },
                ],
                temps: vec![
                    IrTemp {
                        id: TempId(0),
                        ty: IrTypeRef::Boolean,
                    },
                    IrTemp {
                        id: TempId(1),
                        ty: IrTypeRef::Number,
                    },
                    IrTemp {
                        id: TempId(2),
                        ty: IrTypeRef::Object,
                    },
                    IrTemp {
                        id: TempId(3),
                        ty: IrTypeRef::Functor {
                            capture_types: vec![],
                            params: vec![IrTypeRef::Number],
                            ret: Box::new(IrTypeRef::Number),
                        },
                    },
                ],
                return_type: IrTypeRef::Object,
                body: vec![
                    IrInstr::Unary {
                        dst: IrPlace::Temp(TempId(0)),
                        op: IrUnaryOp::Not,
                        value: IrValue::ConstBool(false),
                    },
                    IrInstr::Assign {
                        dst: IrPlace::Temp(TempId(2)),
                        src: IrValue::Null,
                    },
                    IrInstr::SetAttr {
                        object: IrValue::Local(LocalId(0)),
                        attr: AttrId(0),
                        value: IrValue::ConstNumber(1.0),
                    },
                    IrInstr::StaticCall {
                        dst: None,
                        function: "Box_init".to_string(),
                        args: vec![IrValue::Local(LocalId(0))],
                    },
                    IrInstr::BaseCall {
                        dst: Some(IrPlace::Temp(TempId(1))),
                        parent_type: "Parent".to_string(),
                        method: "value".to_string(),
                        args: vec![IrValue::Local(LocalId(0))],
                    },
                    IrInstr::VectorPush {
                        vector: IrValue::Local(LocalId(1)),
                        value: IrValue::Temp(TempId(1)),
                    },
                    IrInstr::VectorGet {
                        dst: IrPlace::Temp(TempId(1)),
                        vector: IrValue::Local(LocalId(1)),
                        index: IrValue::ConstNumber(0.0),
                    },
                    IrInstr::VectorSet {
                        vector: IrValue::Local(LocalId(1)),
                        index: IrValue::ConstNumber(1.0),
                        value: IrValue::Temp(TempId(1)),
                    },
                    IrInstr::MakeClosure {
                        dst: IrPlace::Temp(TempId(3)),
                        function: "lambda_0".to_string(),
                        captures: vec![IrValue::Temp(TempId(1))],
                    },
                    IrInstr::ClosureCall {
                        dst: Some(IrPlace::Temp(TempId(1))),
                        closure: IrValue::Temp(TempId(3)),
                        args: vec![IrValue::ConstNumber(2.0)],
                    },
                    IrInstr::TypeTest {
                        dst: IrPlace::Temp(TempId(0)),
                        value: IrValue::Local(LocalId(0)),
                        type_name: "Box".to_string(),
                    },
                    IrInstr::TypeCast {
                        dst: IrPlace::Temp(TempId(2)),
                        value: IrValue::Local(LocalId(0)),
                        type_name: "Object".to_string(),
                    },
                    IrInstr::Assign {
                        dst: IrPlace::Temp(TempId(2)),
                        src: IrValue::Unit,
                    },
                    IrInstr::Return(Some(IrValue::Temp(TempId(2)))),
                ],
            },
            IrFunction {
                id: FunctionId(1),
                name: "lambda_0".to_string(),
                kind: IrFunctionKind::Lambda,
                params: vec![IrParam {
                    id: ParamId(0),
                    name: "x".to_string(),
                    ty: IrTypeRef::Number,
                }],
                locals: vec![],
                temps: vec![],
                return_type: IrTypeRef::Number,
                body: vec![IrInstr::Return(None)],
            },
        ],
    };

    assert_eq!(
        program.to_string(),
        r#".TYPES
  <empty>

.DATA
data @s0 = 3.5
data @s1 = false

.CODE
entry #0
function entry #0 -> Object {
  local %l0: Box name=obj
  local %l1: Number[] name=items
  temp %t0: Boolean
  temp %t1: Number
  temp %t2: Object
  temp %t3: (Number) -> Number

  %t0 = !false
  %t2 = null
  setattr %l0, #0, 1
  static_call Box_init(%l0)
  %t1 = base_call Parent::value(%l0)
  vector_push %l1, %t1
  %t1 = vector_get %l1[0]
  vector_set %l1[1] = %t1
  %t3 = closure lambda_0 captures [%t1]
  %t1 = closure_call %t3(2)
  %t0 = type_test %l0 is Box
  %t2 = type_cast %l0 as Object
  %t2 = unit
  return %t2
}
lambda lambda_0 #1 -> Number {
  param %p0: Number name=x

  return
}
"#
    );
}
