use serde_json::Value;

pub(crate) fn required_string<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing required string argument: {name}"))
}

pub(crate) fn optional_string(args: &Value, name: &str) -> Result<Option<String>, String> {
    match args.get(name) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("{name} must be a string")),
    }
}

pub(crate) fn optional_nullable_string(args: &Value, name: &str) -> Result<Option<String>, String> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("{name} must be a string or null")),
    }
}

pub(crate) fn optional_bool(args: &Value, name: &str) -> Result<Option<bool>, String> {
    match args.get(name) {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("{name} must be a boolean")),
    }
}

pub(crate) fn string_array(args: &Value, name: &str) -> Result<Option<Vec<String>>, String> {
    match args.get(name) {
        None => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("{name}[{index}] must be a string"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(format!("{name} must be an array of strings")),
    }
}

pub(crate) fn require_confirmation(args: &Value) -> Result<(), String> {
    if args.get("confirm") == Some(&Value::Bool(true)) {
        Ok(())
    } else {
        Err("confirmation required: pass confirm=true".into())
    }
}
