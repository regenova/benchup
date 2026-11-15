//! Several tool calls as one step: `$N` references to earlier results and
//! the JSON report of the whole run. Shared by `benchup call --batch`, MCP
//! `run_batch` on a file and the open app (through `benchup bridge`), so a
//! batch reads and answers the same way everywhere.
use serde_json::{Value, json};

/// Result of applying one or more calls.
pub enum Applied {
    /// Every call succeeded; one result per call.
    Ok(Vec<Value>),
    /// Call `index` failed; nothing was changed or saved; `earlier` holds the results of the calls before it.
    Failed { index: usize, result: Value, earlier: Vec<Value> },
}

/// One step of a reference after `$N`.
pub enum Step<'a> {
    Field(&'a str),
    Index(i64),
}

/// `$N` followed by `.field` and `[index]` steps (N counts calls from 1,
/// negative indexes count from the end), or `None` when `text` is ordinary
/// text such as "$12.50 hardware" or "$5 shelf".
pub fn parse_reference(text: &str) -> Option<(usize, Vec<Step<'_>>)> {
    let body = text.strip_prefix('$')?;
    let digits = body.find(|c: char| !c.is_ascii_digit()).unwrap_or(body.len());
    let call = body[..digits].parse().ok()?;
    let mut rest = &body[digits..];
    let mut steps = Vec::new();
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix('.') {
            let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(tail.len());
            let field = &tail[..end];
            if !field.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
                return None;
            }
            steps.push(Step::Field(field));
            rest = &tail[end..];
        } else if let Some(tail) = rest.strip_prefix('[') {
            let end = tail.find(']')?;
            steps.push(Step::Index(tail[..end].parse().ok()?));
            rest = &tail[end + 1..];
        } else {
            return None;
        }
    }
    Some((call, steps))
}

/// Replaces every `$N.path` reference with that value from an earlier result.
pub fn resolve(value: &Value, results: &[Value]) -> Result<Value, String> {
    match value {
        Value::String(text) => match parse_reference(text) {
            Some((call, steps)) => reference(text, call, &steps, results),
            None => Ok(value.clone()),
        },
        // A reference to a list of ids inside a list is spliced in, so
        // "ids": ["$1.created", "$2.created"] joins both lists. Other lists,
        // such as a point, stay one item.
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let is_reference = matches!(item, Value::String(t) if parse_reference(t).is_some());
                match resolve(item, results)? {
                    Value::Array(inner) if is_reference && inner.iter().all(|v| v.is_u64() || v.is_i64()) => {
                        out.extend(inner)
                    }
                    other => out.push(other),
                }
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| resolve(v, results).map(|v| (k.clone(), v)))
            .collect::<Result<_, _>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

fn reference(text: &str, call: usize, steps: &[Step], results: &[Value]) -> Result<Value, String> {
    let mut current = call
        .checked_sub(1)
        .and_then(|i| results.get(i))
        .ok_or(format!("{text} refers to call {call}, but only {} earlier calls ran", results.len()))?;
    for step in steps {
        let next = match step {
            Step::Field(field) => current.get(field),
            Step::Index(index) => {
                let items = current.as_array().ok_or(format!("{text}: not a list"))?;
                let index = if *index < 0 { items.len() as i64 + index } else { *index };
                usize::try_from(index).ok().and_then(|i| items.get(i))
            }
        };
        current = next.ok_or(format!("{text} does not exist in the result of call {call}"))?;
    }
    Ok(current.clone())
}

/// The JSON answer for applied calls, as the CLI prints it and MCP returns
/// it: one call's result, or `{"ok", "results"}` for a batch. A failed batch
/// leads with the error and hint of the call that failed; the results of the
/// calls before it are kept brief (ids and bounds, no object summaries).
/// `batch` is true for `--batch` and `run_batch`, which always answer with
/// `results`, even for one call.
pub fn report(calls: &[(String, Value)], applied: Applied, batch: bool) -> (Value, bool) {
    match applied {
        Applied::Ok(mut results) if !batch => (results.pop().unwrap_or(Value::Null), true),
        Applied::Ok(results) => (json!({"ok": true, "results": results}), true),
        Applied::Failed { result, .. } if !batch => (result, false),
        Applied::Failed { index, result, mut earlier } => {
            earlier.iter_mut().for_each(brief);
            let mut out = json!({
                "ok": false,
                "failed_call": index + 1,
                "tool": calls[index].0,
                "error": result.get("error").cloned().unwrap_or(Value::Null),
                "result": result,
                "earlier_results": earlier,
                "note": "nothing was changed: earlier_results shows what the calls before it returned, then everything was rolled back; fix this call and run the whole batch again",
            });
            if let Some(hint) = out["result"].get("hint").cloned() {
                out["hint"] = hint;
            }
            (out, false)
        }
    }
}

/// Drops the object summaries that changing tools attach to their results,
/// keeping ids, bounds, errors and the answers of reading tools.
pub fn brief(value: &mut Value) {
    if let Some(map) = value.as_object_mut() {
        if map.contains_key("created") {
            map.remove("objects");
        }
        for key in ["results", "result", "earlier_results"] {
            match map.get_mut(key) {
                Some(Value::Array(items)) => items.iter_mut().for_each(brief),
                Some(item @ Value::Object(_)) => brief(item),
                _ => {}
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_strict_and_only_lists_of_ids_are_spliced() {
        let results = [json!({"created": [7, 8], "center_mm": [1.5, 2.0, 3.0]})];
        let text = json!({"text": "$12.50 hardware", "name": "$5 shelf", "note": "$1.", "cost": "$3.99"});
        assert_eq!(resolve(&text, &results).unwrap(), text);
        let points = json!({"points_mm": ["$1.center_mm", [0, 0, 0]], "ids": ["$1.created", 9]});
        assert_eq!(
            resolve(&points, &results).unwrap(),
            json!({"points_mm": [[1.5, 2.0, 3.0], [0, 0, 0]], "ids": [7, 8, 9]})
        );
        assert_eq!(resolve(&json!("$1.created[-1]"), &results).unwrap(), json!(8));
        assert!(resolve(&json!("$2.created"), &results).is_err(), "a reference to a missing call is an error");
    }

}
