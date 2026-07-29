fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to encode JSON for {}: {error}", path.display()))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn append_transcript(
    path: Option<&Path>,
    command: &str,
    state: Option<&Path>,
    receipt: Option<&Path>,
    seed: Option<u64>,
    args: JsonValue,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let event = json!({
        "kind": "tool_event",
        "command": command,
        "state": state.map(display_path),
        "receipt": receipt.map(display_path),
        "seed": seed,
        "args": args
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    writeln!(file, "{event}")
        .map_err(|error| format!("failed to write transcript {}: {error}", path.display()))
}

fn receipt(
    command: &str,
    seed: Option<u64>,
    input_hash: Option<&str>,
    output_hash: Option<&str>,
    output_ref: Option<&Path>,
    diagnostics: Vec<Diagnostic>,
) -> Receipt {
    Receipt {
        kind: "rusty_procgen.receipt.v1".to_owned(),
        schema_version: 1,
        command: command.to_owned(),
        status: "ok".to_owned(),
        seed,
        input_hash: input_hash.map(str::to_owned),
        output_hash: output_hash.map(str::to_owned),
        output_ref: output_ref.map(display_path),
        diagnostics,
    }
}

fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(format!("fnv1a64:{:016x}", fnv1a64(&bytes)))
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode hash input: {error}"))?;
    Ok(format!("fnv1a64:{:016x}", fnv1a64(&bytes)))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_suffix(seed: u64) -> String {
    format!("{:04x}", seed & 0xffff)
}

fn slugify_label(label: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for character in label.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('_');
            last_was_separator = true;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        "fork".to_owned()
    } else {
        slug
    }
}

fn fatal(
    code: &str,
    node: Option<&str>,
    edge: Option<&str>,
    detail: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        node: node.map(str::to_owned),
        edge: edge.map(str::to_owned),
        detail: detail.into(),
        repair_hint: None,
    }
}

fn fatal_with_hint(
    code: &str,
    node: Option<&str>,
    edge: Option<&str>,
    detail: impl Into<String>,
    repair_hint: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        node: node.map(str::to_owned),
        edge: edge.map(str::to_owned),
        detail: detail.into(),
        repair_hint: Some(repair_hint.into()),
    }
}

fn warning_with_hint(
    code: &str,
    node: Option<&str>,
    edge: Option<&str>,
    detail: impl Into<String>,
    repair_hint: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Warning,
        node: node.map(str::to_owned),
        edge: edge.map(str::to_owned),
        detail: detail.into(),
        repair_hint: Some(repair_hint.into()),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

