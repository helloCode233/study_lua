use crate::vm::model::Vm;
use crate::{Value, VmError};

impl Vm {
    pub(crate) fn oob(index: usize, len: usize) -> VmError {
        VmError::StackOutOfBounds { index, len }
    }

    pub(crate) fn proto_oob(index: usize, len: usize) -> VmError {
        VmError::ProtoOutOfBounds { index, len }
    }

    pub(crate) fn as_number(v: &Value) -> Result<f64, VmError> {
        match v {
            Value::Number(n) => Ok(*n),
            _ => Err(VmError::TypeError {
                expected: "number",
                got: v.clone(),
            }),
        }
    }
}
