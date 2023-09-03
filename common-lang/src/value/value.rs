use crate::ast::*;
use crate::common_derives;
use crate::ops::{BinOpKind, UnOpKind};
use crate::value::{StructType, TypeBounds, TypeValue};
use common::*;
use serde_json::json;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};

pub trait ToJson {
    fn to_json(&self) -> Result<serde_json::Value>;
    fn to_value<T: DeserializeOwned>(&self) -> Result<T>
    where
        Self: Sized,
    {
        let json = self.to_json()?;
        Ok(serde_json::from_value(json)?)
    }
}
/// wrap struct declare with derive Debug, Clone, Serialize, Deserialize,
/// PartialEq, Eq,
/// Hash, PartialOrd, Ord
macro_rules! plain_value {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name;
        impl ToJson for $name {
            fn to_json(&self) -> Result<serde_json::Value> {
                Ok(json!(null))
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($name))
            }
        }
    };

    ($(#[$attr:meta])* $name:ident: $ty:ty) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name {
            pub value: $ty,
        }
        impl $name {
            pub fn new(v: $ty) -> Self {
                Self { value: v }
            }
        }
        impl ToJson for $name {
            fn to_json(&self) -> Result<serde_json::Value> {
                Ok(json!(self.value))
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.value)
            }
        }
    };
}

common_derives! {
    pub enum Value {
        Int(IntValue),
        Bool(BoolValue),
        Decimal(DecimalValue),
        Char(CharValue),
        String(StringValue),
        List(ListValue),
        Unit(UnitValue),
        Null(NullValue),
        Undefined(UndefinedValue),
        Type(TypeValue),
        Struct(StructValue),
        Structural(StructuralStructValue),
        Function(FunctionValue),
        Tuple(TupleValue),
        Expr(Box<Expr>),
        BinOpKind(BinOpKind),
        UnOpKind(UnOpKind),
        Any(AnyBox),
    }
}
impl Value {
    pub fn bool(b: bool) -> Value {
        Value::Bool(BoolValue::new(b))
    }
    pub fn decimal(d: f64) -> Value {
        Value::Decimal(DecimalValue::new(d))
    }
    pub fn int(i: i64) -> Value {
        Value::Int(IntValue::new(i))
    }
    pub fn unit() -> Value {
        Value::Unit(UnitValue)
    }
    pub fn null() -> Value {
        Value::Null(NullValue)
    }
    pub fn expr(e: Expr) -> Self {
        match e {
            Expr::Value(v) => v,
            _ => Value::Expr(Box::new(e)),
        }
    }
    pub fn any<T: AnyBoxable>(any: T) -> Self {
        Self::Any(AnyBox::new(any))
    }
    pub fn undefined() -> Self {
        Self::Undefined(UndefinedValue)
    }
}
impl ToJson for Value {
    fn to_json(&self) -> Result<serde_json::Value> {
        match self {
            Value::Int(i) => i.to_json(),
            Value::Bool(b) => b.to_json(),
            Value::Decimal(d) => d.to_json(),
            Value::Char(c) => c.to_json(),
            Value::String(s) => s.to_json(),
            Value::List(l) => l.to_json(),
            Value::Unit(u) => u.to_json(),
            Value::Null(n) => n.to_json(),
            Value::Undefined(u) => u.to_json(),
            Value::Struct(s) => s.to_json(),
            Value::Tuple(t) => t.to_json(),
            _ => bail!("cannot convert value to json: {:?}", self),
        }
    }
}
impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(i) => Display::fmt(i, f),
            Value::Bool(b) => Display::fmt(b, f),
            Value::Decimal(d) => Display::fmt(d, f),
            Value::Char(c) => Display::fmt(c, f),
            Value::String(s) => Display::fmt(s, f),
            Value::List(l) => Display::fmt(l, f),
            Value::Unit(u) => Display::fmt(u, f),
            Value::Null(n) => Display::fmt(n, f),
            Value::Undefined(u) => Display::fmt(u, f),
            Value::Struct(s) => Display::fmt(s, f),
            Value::Tuple(t) => Display::fmt(t, f),
            _ => panic!("cannot display value: {:?}", self),
        }
    }
}
plain_value! {
    IntValue: i64
}
plain_value! {
    BoolValue: bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecimalValue {
    pub value: f64,
}
impl PartialEq for DecimalValue {
    fn eq(&self, other: &Self) -> bool {
        self.value.total_cmp(&other.value) == std::cmp::Ordering::Equal
    }
}

impl Eq for DecimalValue {}
impl PartialOrd for DecimalValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.value.total_cmp(&other.value))
    }
}
impl Ord for DecimalValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.total_cmp(&other.value)
    }
}
impl Hash for DecimalValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.to_bits().hash(state);
    }
}
impl DecimalValue {
    pub fn new(v: f64) -> Self {
        Self { value: v }
    }
}
impl ToJson for DecimalValue {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(self.value))
    }
}
impl Display for DecimalValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

plain_value! {
    CharValue: char
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StringValue {
    pub value: String,
    pub owned: bool,
}

impl StringValue {
    pub fn new_owned(s: impl Into<String>) -> Self {
        Self {
            value: s.into(),
            owned: true,
        }
    }
    pub fn new_ref(s: impl Into<String>) -> Self {
        Self {
            value: s.into(),
            owned: false,
        }
    }
}

impl ToJson for StringValue {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(self.value))
    }
}

impl Display for StringValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}
common_derives! {
    pub struct ListValue {
        pub values: Vec<Value>,
    }
}
impl ListValue {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}
impl ToJson for ListValue {
    fn to_json(&self) -> Result<serde_json::Value> {
        let values: Vec<_> = self.values.iter().map(|x| x.to_json()).try_collect()?;
        Ok(json!(values))
    }
}
impl Display for ListValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for value in &self.values {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}", value)?;
        }
        write!(f, "]")
    }
}
plain_value!(UnitValue);
plain_value!(NullValue);
plain_value!(UndefinedValue);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FieldValue {
    pub name: Ident,
    pub value: Value,
}
impl FieldValue {
    pub fn new(name: Ident, value: Value) -> Self {
        Self { name, value }
    }
}

common_derives! {
    pub struct StructValue {
        pub struct_: StructType,
        pub fields: Vec<FieldValue>,
    }
}
impl StructValue {
    pub fn new(struct_: StructType, fields: Vec<FieldValue>) -> Self {
        Self { struct_, fields }
    }
}
impl ToJson for StructValue {
    fn to_json(&self) -> Result<serde_json::Value> {
        let mut map = serde_json::Map::new();
        for field in &self.fields {
            map.insert(field.name.name.clone(), field.value.to_json()?);
        }
        Ok(json!(map))
    }
}
impl Display for StructValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for field in &self.fields {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}: {}", field.name, field.value)?;
        }
        write!(f, "}}")
    }
}
common_derives! {
    pub struct StructuralStructValue {
        pub fields: Vec<FieldValue>,
    }
}
impl StructuralStructValue {
    pub fn new(fields: Vec<FieldValue>) -> Self {
        Self { fields }
    }
}
impl ToJson for StructuralStructValue {
    fn to_json(&self) -> Result<serde_json::Value> {
        let mut map = serde_json::Map::new();
        for field in &self.fields {
            map.insert(field.name.name.clone(), field.value.to_json()?);
        }
        Ok(json!(map))
    }
}
impl Display for StructuralStructValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for field in &self.fields {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}: {}", field.name, field.value)?;
        }
        write!(f, "}}")
    }
}

common_derives! {
    pub struct FunctionParam {
        pub name: Ident,
        pub ty: TypeValue,
    }
}

common_derives! {
    pub struct GenericParam {
        pub name: Ident,
        pub bounds: TypeBounds,
    }

}

common_derives! {
    pub struct FunctionValue {
        pub name: Option<Ident>,
        pub params: Vec<FunctionParam>,
        pub generics_params: Vec<GenericParam>,
        pub ret: TypeValue,
        pub body: Box<Expr>,
    }

}
impl FunctionValue {
    pub fn is_runtime_only(&self) -> bool {
        self.generics_params.is_empty()
    }
}
common_derives! {
    pub struct TupleValue {
        pub values: Vec<Value>,
    }
}
impl TupleValue {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}

impl ToJson for TupleValue {
    fn to_json(&self) -> Result<serde_json::Value> {
        let values: Vec<_> = self.values.iter().map(|x| x.to_json()).try_collect()?;
        Ok(json!(values))
    }
}
impl Display for TupleValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "(")?;
        let mut first = true;
        for value in &self.values {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}", value)?;
        }
        write!(f, ")")
    }
}
