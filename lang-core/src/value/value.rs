use crate::ast::BExpr;
use crate::id::Ident;
use crate::utils::to_json::ToJson;
use crate::value::{Type, TypeBounds, TypeStruct, Value};
use crate::{common_enum, common_struct, get_threadlocal_serializer};
use bytes::BytesMut;
use common::*;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Add, Deref, DerefMut, Mul, Sub};

/// wrap struct declare with derive Debug, Clone, Serialize, Deserialize,
/// PartialEq, Eq,
/// Hash, PartialOrd, Ord
macro_rules! plain_value {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name;
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($name))
            }
        }
    };
    (no_ord $(#[$attr:meta])* $name:ident: $ty:ty) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    ($(#[$attr:meta])* $name:ident: $ty:ty) => {
        plain_value!(no_ord $(#[$attr])* #[derive(PartialOrd, Ord)] $name: $ty);
    };
}

plain_value! {
    ValueInt: i64
}
plain_value! {
    ValueBool: bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueDecimal {
    pub value: f64,
}
impl PartialEq for ValueDecimal {
    fn eq(&self, other: &Self) -> bool {
        self.value.total_cmp(&other.value) == std::cmp::Ordering::Equal
    }
}

impl Eq for ValueDecimal {}
impl PartialOrd for ValueDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.value.total_cmp(&other.value))
    }
}
impl Ord for ValueDecimal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.total_cmp(&other.value)
    }
}
impl Hash for ValueDecimal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.to_bits().hash(state);
    }
}
impl ValueDecimal {
    pub fn new(v: f64) -> Self {
        Self { value: v }
    }
}
impl ToJson for ValueDecimal {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(self.value))
    }
}
impl Display for ValueDecimal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

plain_value! {
    ValueChar: char
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ValueString {
    pub value: String,
    pub owned: bool,
}

impl ValueString {
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

impl ToJson for ValueString {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(self.value))
    }
}

impl Display for ValueString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}
common_struct! {
    pub struct ValueList {
        pub values: Vec<Value>,
    }
}
impl ValueList {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}
impl ToJson for ValueList {
    fn to_json(&self) -> Result<serde_json::Value> {
        let values: Vec<_> = self.values.iter().map(|x| x.to_json()).try_collect()?;
        Ok(json!(values))
    }
}
impl Display for ValueList {
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

common_struct! {
    pub struct ValueBytes {
        pub value: BytesMut,
    }
}
impl ValueBytes {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            value: BytesMut::with_capacity(capacity),
        }
    }
    pub fn zeroed(len: usize) -> Self {
        Self {
            value: BytesMut::zeroed(len),
        }
    }
    pub fn new(value: BytesMut) -> Self {
        Self { value }
    }
}
impl<T: Into<BytesMut>> From<T> for ValueBytes {
    fn from(values: T) -> Self {
        Self::new(values.into())
    }
}
impl Deref for ValueBytes {
    type Target = BytesMut;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl DerefMut for ValueBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
common_enum! {
    #[derive(Copy, PartialOrd, Ord)]
    pub enum ValuePointerKind {
        Unspecified,
        Managed,
        Escaped,
    }
}
common_struct! {
    #[derive(Copy, PartialOrd, Ord)]
    pub struct ValuePointer {
        pub value: i64,
        pub kind: ValuePointerKind,
    }
}
impl ValuePointer {
    pub fn new(value: i64) -> Self {
        Self {
            value,
            kind: ValuePointerKind::Unspecified,
        }
    }
    pub fn managed(value: i64) -> Self {
        Self {
            value,
            kind: ValuePointerKind::Managed,
        }
    }
    pub fn escaped(value: *const u8) -> Self {
        Self {
            value: value as _,
            kind: ValuePointerKind::Escaped,
        }
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.value as _
    }
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.value as _
    }
}
impl Display for ValuePointer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ptr({})", self.value)
    }
}
impl Add<ValueOffset> for ValuePointer {
    type Output = Self;

    fn add(self, rhs: ValueOffset) -> Self::Output {
        Self {
            value: self.value + rhs.value,
            kind: self.kind,
        }
    }
}
impl Sub<ValuePointer> for ValuePointer {
    type Output = ValueOffset;

    fn sub(self, rhs: Self) -> Self::Output {
        ValueOffset {
            value: self.value - rhs.value,
        }
    }
}
impl Sub<ValueOffset> for ValuePointer {
    type Output = Self;

    fn sub(self, rhs: ValueOffset) -> Self::Output {
        Self {
            value: self.value - rhs.value,
            kind: self.kind,
        }
    }
}
plain_value!(ValueOffset: i64);

impl Add<ValueOffset> for ValueOffset {
    type Output = Self;

    fn add(self, rhs: ValueOffset) -> Self::Output {
        Self {
            value: self.value + rhs.value,
        }
    }
}
impl Sub<ValueOffset> for ValueOffset {
    type Output = Self;

    fn sub(self, rhs: ValueOffset) -> Self::Output {
        Self {
            value: self.value - rhs.value,
        }
    }
}
impl Add<ValuePointer> for ValueOffset {
    type Output = ValuePointer;

    fn add(self, rhs: ValuePointer) -> Self::Output {
        ValuePointer {
            value: self.value + rhs.value,
            kind: rhs.kind,
        }
    }
}
impl Mul<ValueInt> for ValueOffset {
    type Output = Self;

    fn mul(self, rhs: ValueInt) -> Self::Output {
        Self {
            value: self.value * rhs.value,
        }
    }
}
plain_value!(ValueUnit);
impl ToJson for ValueUnit {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!({}))
    }
}

plain_value!(ValueNull);
impl ToJson for ValueNull {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(null))
    }
}
plain_value!(ValueUndefined);

common_struct! {
    pub struct ValueEscaped {
        pub ptr: ValuePointer,
        pub size: i64,
        pub align: i64,
        _priv: ()
    }
}
impl ValueEscaped {
    pub fn new(size: i64, align: i64) -> Self {
        let layout = std::alloc::Layout::from_size_align(size as _, align as _).unwrap();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        Self {
            ptr: ValuePointer::escaped(ptr),
            size,
            align,
            _priv: (),
        }
    }
    fn as_layout(&self) -> std::alloc::Layout {
        std::alloc::Layout::from_size_align(self.size as _, self.align as _).unwrap()
    }
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.size as _) }
    }
    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_mut_ptr(), self.size as _) }
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_mut_ptr()
    }
    pub unsafe fn drop_in_place<T>(&mut self) {
        std::ptr::drop_in_place(self.as_mut_ptr() as *mut T);
    }
}
impl Drop for ValueEscaped {
    fn drop(&mut self) {
        let layout = self.as_layout();
        unsafe { std::alloc::dealloc(self.ptr.as_mut_ptr(), layout) }
    }
}

impl ToJson for ValueUndefined {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(null))
    }
}
plain_value!(ValueNone);
impl ToJson for ValueNone {
    fn to_json(&self) -> Result<serde_json::Value> {
        Ok(json!(null))
    }
}

common_struct! {
    pub struct ValueSome {
        pub value: Box<Value>,
    }
}
impl ValueSome {
    pub fn new(value: Value) -> Self {
        Self {
            value: value.into(),
        }
    }
}
impl ToJson for ValueSome {
    fn to_json(&self) -> Result<serde_json::Value> {
        self.value.to_json()
    }
}
common_struct! {
    pub struct ValueOption {
        pub value: Option<Box<Value>>,
    }
}

impl ValueOption {
    pub fn new(value: Option<Value>) -> Self {
        Self {
            value: value.map(|x| x.into()),
        }
    }
}
impl ToJson for ValueOption {
    fn to_json(&self) -> Result<serde_json::Value> {
        match &self.value {
            Some(v) => v.to_json(),
            None => Ok(json!(null)),
        }
    }
}
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

common_struct! {
    pub struct ValueStruct {
        pub ty: TypeStruct,
        pub structural: ValueStructural
    }
}
impl ValueStruct {
    pub fn new(ty: TypeStruct, fields: Vec<FieldValue>) -> Self {
        Self {
            ty,
            structural: ValueStructural { fields },
        }
    }
}
impl ToJson for ValueStruct {
    fn to_json(&self) -> Result<serde_json::Value> {
        self.structural.to_json()
    }
}
impl Display for ValueStruct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ty.name)?;
        write!(f, "{{")?;
        let mut first = true;
        for field in &self.structural.fields {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}: {}", field.name, field.value)?;
        }
        write!(f, "}}")
    }
}
common_struct! {
    pub struct ValueStructural {
        pub fields: Vec<FieldValue>,
    }
}
impl ValueStructural {
    pub fn new(fields: Vec<FieldValue>) -> Self {
        Self { fields }
    }
    pub fn get_field(&self, name: &Ident) -> Option<&FieldValue> {
        self.fields.iter().find(|x| &x.name == name)
    }
}
impl ToJson for ValueStructural {
    fn to_json(&self) -> Result<serde_json::Value> {
        let mut map = serde_json::Map::new();
        for field in &self.fields {
            map.insert(field.name.name.clone(), field.value.to_json()?);
        }

        Ok(json!(map))
    }
}
impl Display for ValueStructural {
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

common_struct! {
    pub struct FunctionParam {
        pub name: Ident,
        pub ty: Type,
    }
}

common_struct! {
    pub struct GenericParam {
        pub name: Ident,
        pub bounds: TypeBounds,
    }

}
common_struct! {
    pub struct FunctionSignature {
        pub name: Option<Ident>,
        pub params: Vec<FunctionParam>,
        pub generics_params: Vec<GenericParam>,
        pub ret: Type,
    }
}

common_struct! {
    pub struct ValueFunction {
        pub sig: FunctionSignature,
        pub body: BExpr,
    }
}
impl ValueFunction {
    pub fn is_runtime_only(&self) -> bool {
        self.generics_params.is_empty()
    }
}
impl Deref for ValueFunction {
    type Target = FunctionSignature;

    fn deref(&self) -> &Self::Target {
        &self.sig
    }
}
impl DerefMut for ValueFunction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sig
    }
}
impl Display for ValueFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = get_threadlocal_serializer()
            .serialize_function(self)
            .unwrap();
        f.write_str(&s)
    }
}
common_struct! {
    pub struct ValueTuple {
        pub values: Vec<Value>,
    }
}
impl ValueTuple {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}

impl ToJson for ValueTuple {
    fn to_json(&self) -> Result<serde_json::Value> {
        let values: Vec<_> = self.values.iter().map(|x| x.to_json()).try_collect()?;
        Ok(json!(values))
    }
}
impl Display for ValueTuple {
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
