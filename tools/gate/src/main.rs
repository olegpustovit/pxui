use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Clone, Debug)]
struct Token {
    text: String,
    line: usize,
}

fn advance_lines(bytes: &[u8], first: usize, last: usize, line: &mut usize) {
    *line += bytes[first..last].iter().filter(|&&byte| byte == b'\n').count();
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote = start;
    if bytes.get(quote) == Some(&b'b') || bytes.get(quote) == Some(&b'c') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'r') {
        return None;
    }
    quote += 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - start - if bytes.get(start) == Some(&b'b') || bytes.get(start) == Some(&b'c') { 2 } else { 1 };
    let mut cursor = quote + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && bytes.get(cursor + 1..cursor + 1 + hashes).is_some_and(|ending| ending.iter().all(|&byte| byte == b'#'))
        {
            return Some(cursor + 1 + hashes);
        }
        cursor += 1;
    }
    Some(bytes.len())
}

fn lex(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < bytes.len() {
        let byte = bytes[i];
        if byte.is_ascii_whitespace() {
            if byte == b'\n' {
                line += 1;
            }
            i += 1;
            continue;
        }
        if bytes.get(i..i + 2) == Some(&b"//"[..]) {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes.get(i..i + 2) == Some(&b"/*"[..]) {
            let start = i;
            i += 2;
            let mut depth = 1usize;
            while i < bytes.len() && depth > 0 {
                match bytes.get(i..i + 2) {
                    Some(pair) if pair == &b"/*"[..] => {
                        depth += 1;
                        i += 2;
                    }
                    Some(pair) if pair == &b"*/"[..] => {
                        depth -= 1;
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            advance_lines(bytes, start, i, &mut line);
            continue;
        }
        if let Some(end) = raw_string_end(bytes, i) {
            advance_lines(bytes, i, end, &mut line);
            i = end;
            continue;
        }
        if byte == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            advance_lines(bytes, start, i, &mut line);
            continue;
        }
        if byte == b'\'' {
            let mut end = i + 1;
            if end < bytes.len() && bytes[end] == b'\\' {
                end = (end + 2).min(bytes.len());
            } else if end < bytes.len() {
                end += if bytes[end].is_ascii() { 1 } else { source[end..].chars().next().map_or(1, char::len_utf8) };
            }
            if bytes.get(end) == Some(&b'\'') {
                end += 1;
                advance_lines(bytes, i, end, &mut line);
                i = end;
                continue;
            }
        }
        if byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80 {
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] >= 0x80)
            {
                i += 1;
            }
            tokens.push(Token { text: source[start..i].to_owned(), line });
            continue;
        }
        let start = i;
        let pair = bytes.get(i..i + 2);
        if pair == Some(&b"::"[..]) || pair == Some(&b"->"[..]) || pair == Some(&b"=>"[..]) {
            i += 2;
        } else {
            i += 1;
        }
        tokens.push(Token { text: source[start..i].to_owned(), line });
    }
    tokens
}

fn matching(tokens: &[Token], start: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if token.text == open {
            depth += 1;
        } else if token.text == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn generic_end(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.text.as_str() {
            "<" => depth += 1,
            ">" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            ";" | "{" if depth == 1 => return None,
            _ => (),
        }
    }
    None
}

fn issue(issues: &mut Vec<String>, path: &str, line: usize, message: impl AsRef<str>) {
    issues.push(format!("{path}:{line}: {}", message.as_ref()));
}

fn shape_source(crate_name: &str, path: &str, source: &str) -> Vec<String> {
    let tokens = lex(source);
    // Whole test files (tests/, src/tests.rs, src/test_*.rs) count as one test region.
    let file = path.rsplit('/').next().unwrap_or(path);
    let test_file = path.contains("/tests/") || file == "tests.rs" || file.starts_with("test_");
    let test_ranges = if test_file { vec![(0, tokens.len())] } else { test_regions(&tokens) };
    // Only the value-returning worker-context helper may use this generic signature.
    // Keep the exception in its header; sibling and nested generic items still fail.
    let domain_header = if crate_name == "parallel" && matches!(path, "parallel/src/lib.rs" | "crates/parallel/src/lib.rs") {
        let signature = lex("fn with_domains<R>(domains: i64, work: impl FnOnce() -> R) -> R");
        tokens.iter().enumerate().find_map(|(index, token)| {
            if token.text != "fn" { return None; }
            let (body, _) = function_body(&tokens, index)?;
            tokens[index..body].iter().map(|token| &token.text).eq(signature.iter().map(|token| &token.text)).then_some((index, body))
        })
    } else { None };
    // Per-element kernel entry points take `impl Fn` so the body inlines into the loop (Z3);
    // allowlisted by path and name, exactly like with_domains.
    const KERNEL_ENTRY: &[(&str, &str)] = &[
        ("rdk_core/src/support.rs", "parallel_for"),
        ("rdk_core/src/support.rs", "par_fill"),
        ("rdk_core/src/group.rs", "init"),
        ("rdk_core/src/edge_group.rs", "init"),
        ("flow_sop/src/attribute_kernel.rs", "ranges"),
    ];
    let kernel_headers: Vec<(usize, usize)> = tokens.iter().enumerate().filter_map(|(index, token)| {
        if token.text != "fn" { return None; }
        let name = tokens.get(index + 1)?;
        KERNEL_ENTRY.iter().any(|(file, entry)| path.ends_with(file) && name.text == *entry)
            .then(|| function_body(&tokens, index).map(|(body, _)| (index, body)))?
    }).collect();
    let unsafe_allowed = crate_name.ends_with("_sys") || policy::UNSAFE_CRATES.contains(&crate_name)
        || policy::UNSAFE_FILES.iter().any(|(file, _)| *file == path);
    let mut issues = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if token.text == "unsafe" && !unsafe_allowed {
            issue(&mut issues, path, token.line, "unsafe is restricted to *_sys crates and the policy's UNSAFE_CRATES");
        }
        if matches!(token.text.as_str(), "async" | "await") {
            issue(&mut issues, path, token.line, "async/await is not allowed in product code");
        }
        if token.text == "catch_unwind" && crate_name != "parallel" && !policy::wrapper_allows(path, "catch_unwind", &[]) {
            issue(&mut issues, path, token.line, "catch_unwind is restricted to parallel worker isolation and the bin/rays crash reporter");
        }
        if token.text == "macro_rules"
            && tokens.get(index + 1).is_some_and(|next| next.text == "!")
        {
            issue(&mut issues, path, token.line, "macro_rules! is not allowed in product crates");
        }
        // unreachable! is OCaml's `assert false` (an impossible case, never control flow) and is allowed.
        if matches!(token.text.as_str(), "panic" | "todo" | "unimplemented")
            && tokens.get(index + 1).is_some_and(|next| next.text == "!")
            && !test_ranges.iter().any(|(start, end)| *start <= index && index <= *end)
        {
            issue(&mut issues, path, token.line, "panic macros must not be used for control flow");
        }
        if token.text == "impl" {
            let previous = index.checked_sub(1).and_then(|previous| tokens.get(previous)).map(|token| token.text.as_str());
            let lifetime_reference = index >= 3 && tokens[index - 2].text == "'" && tokens[index - 3].text == "&";
            if (matches!(previous, Some(":") | Some("->") | Some("=") | Some("<") | Some(",") | Some("(") | Some("&") | Some("mut") | Some(">"))
                || lifetime_reference)
                && !domain_header.is_some_and(|(start, body)| start <= index && index < body)
                && !kernel_headers.iter().any(|(start, body)| *start <= index && index < *body)
            {
                issue(&mut issues, path, token.line, "impl Trait is not allowed in product crates");
            }
        }
        if token.text == "proc_macro" || token.text == "proc_macro_derive" || token.text == "proc_macro_attribute" {
            issue(&mut issues, path, token.line, "procedural macros are not allowed in product crates");
        }
    }

    for index in 0..tokens.len() {
        let token = &tokens[index];
        let item_name = match token.text.as_str() {
            "fn" | "struct" | "enum" | "union" | "trait" | "type" => {
                tokens.get(index + 1).filter(|name| is_identifier(&name.text)).map(|_| index + 1)
            }
            "impl" => Some(index),
            _ => None,
        };
        if let Some(name_index) = item_name {
            let generic_index = if token.text == "impl" { index + 1 } else { name_index + 1 };
            if tokens.get(generic_index).is_some_and(|next| next.text == "<") {
                // Borrow lifetimes do not monomorphize functions or impls; only type parameters do.
                let borrow_lifetimes_only = matches!(token.text.as_str(), "fn" | "impl")
                    && generic_end(&tokens, generic_index).is_some_and(|end| {
                        let parameters = &tokens[generic_index + 1..end];
                        !parameters.is_empty() && parameters.iter().enumerate().all(|(at, parameter)| {
                            matches!(parameter.text.as_str(), "'" | ":" | "+" | ",")
                                || (at > 0 && parameters[at - 1].text == "'" && is_identifier(&parameter.text))
                        })
                    });
                let wrapper_closure = generic_end(&tokens, generic_index).is_some_and(|end| {
                    let parameters: Vec<&str> = tokens[generic_index + 1..end].iter().map(|part| part.text.as_str()).collect();
                    policy::wrapper_allows(path, "generic", &parameters)
                });
                if !borrow_lifetimes_only && !wrapper_closure && !domain_header.is_some_and(|(start, _)| token.text == "fn" && start == index)
                    && !kernel_headers.iter().any(|(start, _)| token.text == "fn" && *start == index) {
                    issue(&mut issues, path, token.line, "generic items are not allowed in product crates");
                }
                if matches!(token.text.as_str(), "struct" | "enum") {
                    if let Some(end) = generic_end(&tokens, generic_index) {
                        if tokens[generic_index + 1..end].iter().any(|parameter| parameter.text == "'") && !policy::wrapper_allows(path, "struct lifetime", &[]) {
                            issue(&mut issues, path, token.line, "struct and enum lifetime parameters are not allowed");
                        }
                    }
                }
            }
        }

        if token.text == "#" {
            let bracket = if tokens.get(index + 1).is_some_and(|next| next.text == "!") { index + 2 } else { index + 1 };
            if tokens.get(bracket).is_some_and(|next| next.text == "[") {
                if let Some(end) = matching(&tokens, bracket, "[", "]") {
                    let attribute = &tokens[bracket + 1..end];
                    if attribute.first().is_some_and(|part| part.text == "ignore")
                        || attribute.first().is_some_and(|part| part.text == "cfg_attr") && attribute.iter().any(|part| part.text == "ignore") {
                        issue(&mut issues, path, token.line, "ignored tests are forbidden; fix, shrink or delete the case");
                    }
                    for (offset, part) in attribute.iter().enumerate() {
                        if part.text == "derive" && attribute.get(offset + 1).is_some_and(|next| next.text == "(") {
                            if let Some(derive_end) = matching(attribute, offset + 1, "(", ")") {
                                for derive in &attribute[offset + 2..derive_end] {
                                    if is_identifier(&derive.text)
                                        && !matches!(derive.text.as_str(), "Clone" | "Copy" | "Debug" | "PartialEq" | "Eq" | "Hash" | "Default")
                                    {
                                        issue(&mut issues, path, derive.line, format!("derive `{}` is not allowed", derive.text));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    issues.extend(hashmap_output_issues(path, &tokens));
    issues
}

fn is_identifier(value: &str) -> bool {
    value.chars().next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || !first.is_ascii())
}

fn hash_map_names(tokens: &[Token]) -> HashSet<String> {
    let mut names = HashSet::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "HashMap" {
            continue;
        }
        for cursor in (index.saturating_sub(12)..index).rev() {
            match tokens[cursor].text.as_str() {
                ";" | "{" | "}" | "," => break,
                ":" => {
                    if let Some(name) = cursor.checked_sub(1).and_then(|name| tokens.get(name)) {
                        if is_identifier(&name.text) {
                            names.insert(name.text.clone());
                        }
                    }
                    break;
                }
                "let" => {
                    let name_index = if cursor > 0 && tokens[cursor - 1].text == "mut" { cursor - 2 } else { cursor - 1 };
                    if let Some(name) = tokens.get(name_index) {
                        if is_identifier(&name.text) {
                            names.insert(name.text.clone());
                        }
                    }
                    break;
                }
                _ => (),
            }
        }
    }
    names
}

fn function_body(tokens: &[Token], start: usize) -> Option<(usize, usize)> {
    if !tokens.get(start + 1).is_some_and(|name| is_identifier(&name.text)) {
        return None;
    }
    let mut cursor = start + 2;
    while cursor < tokens.len() {
        match tokens[cursor].text.as_str() {
            ";" => return None,
            "{" => return matching(tokens, cursor, "{", "}").map(|end| (cursor, end)),
            "(" => cursor = matching(tokens, cursor, "(", ")")? + 1,
            "[" => cursor = matching(tokens, cursor, "[", "]")? + 1,
            _ => cursor += 1,
        }
    }
    None
}

fn test_regions(tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "#" || !tokens.get(index + 1).is_some_and(|next| next.text == "[") {
            continue;
        }
        let Some(attribute_end) = matching(tokens, index + 1, "[", "]") else { continue };
        let attribute = &tokens[index + 2..attribute_end];
        let test_fn = attribute.len() == 1 && attribute[0].text == "test";
        let test_module = attribute.iter().any(|part| part.text == "cfg")
            && attribute.iter().any(|part| part.text == "test")
            && !attribute.iter().any(|part| part.text == "not");
        if !test_fn && !test_module {
            continue;
        }
        let item_start = attribute_end + 1;
        if test_fn {
            if let Some(function) = (item_start..tokens.len().min(item_start + 12)).find(|&cursor| tokens[cursor].text == "fn") {
                if let Some((start, end)) = function_body(tokens, function) {
                    ranges.push((start, end));
                }
            }
        } else if let Some(module) = (item_start..tokens.len().min(item_start + 12)).find(|&cursor| tokens[cursor].text == "mod") {
            if let Some(open) = (module + 1..tokens.len().min(module + 12)).find(|&cursor| tokens[cursor].text == "{") {
                if let Some(end) = matching(tokens, open, "{", "}") {
                    ranges.push((open, end));
                }
            }
        }
    }
    ranges
}


fn emits_text(name: &str, signature_and_body: &[Token]) -> bool {
    let lower = name.to_ascii_lowercase();
    if ["emit", "print", "write", "serialize", "encode", "format", "to_string", "display", "render"]
        .iter().any(|word| lower.contains(word))
    {
        return true;
    }
    signature_and_body.windows(2).any(|pair| {
        matches!(pair[0].text.as_str(), "format" | "write" | "writeln" | "print" | "println") && pair[1].text == "!"
    }) || signature_and_body.windows(2).any(|pair| pair[0].text == "String" && pair[1].text == "::")
        || signature_and_body.iter().any(|token| {
            matches!(token.text.as_str(), "push_str" | "write_str" | "write_fmt" | "to_string")
        })
}

// designed limit (specification/pxui.md): output detection is token-based; switch to syntax-aware provenance if new emission idioms appear.
fn hashmap_output_issues(path: &str, tokens: &[Token]) -> Vec<String> {
    let names = hash_map_names(tokens);
    if names.is_empty() {
        return Vec::new();
    }
    let mut issues = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "fn" {
            continue;
        }
        let Some(name) = tokens.get(index + 1).map(|name| name.text.as_str()) else { continue };
        let Some((body_start, body_end)) = function_body(tokens, index) else { continue };
        let signature = &tokens[index..=body_end];
        let output = emits_text(name, signature);
        if !output {
            continue;
        }
        let iterates_map = (body_start..body_end).any(|cursor| {
            if matches!(tokens[cursor].text.as_str(), "iter" | "keys" | "values" | "into_iter" | "into_keys" | "into_values" | "drain")
                && tokens.get(cursor + 1).is_some_and(|next| next.text == "(")
                && cursor >= 2
                && tokens[cursor - 1].text == "."
            {
                let receiver = &tokens[cursor - 2].text;
                if names.contains(receiver) {
                    return true;
                }
                if cursor >= 4 && tokens[cursor - 3].text == "." && tokens[cursor - 4].text == "self" {
                    return names.contains(receiver);
                }
            }
            if tokens[cursor].text == "in" {
                let mut value = cursor + 1;
                while value < body_end && matches!(tokens[value].text.as_str(), "&" | "*" | "(") {
                    value += 1;
                }
                if value < body_end && names.contains(&tokens[value].text) {
                    return true;
                }
            }
            false
        });
        if iterates_map {
            issue(&mut issues, path, token.line, "HashMap iteration must not determine emitted output");
        }
    }
    issues
}

fn rust_sources(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if matches!(name.to_str(), Some("target" | "_build" | ".git")) {
                continue;
            }
            rust_sources(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn lib_unit_tests_disabled(workspace: &Path) -> Result<std::collections::HashSet<String>, String> {
    let mut manifests = Vec::new();
    cargo_manifests(&workspace.join("crates"), &mut manifests)?;
    let mut disabled = std::collections::HashSet::new();
    for manifest in manifests {
        let parsed = parse_manifest(&manifest)?;
        let Some(package_name) = parsed.package_name.as_deref() else { continue };
        if !parsed.lib_unit_tests {
            disabled.insert(crate_name(package_name, parsed.lib_name.as_deref()));
        }
    }
    Ok(disabled)
}

fn shape(workspace: &Path) -> Result<Vec<String>, String> {
    let crates = workspace.join("crates");
    let disabled_lib_tests = lib_unit_tests_disabled(workspace)?;
    let mut files = Vec::new();
    rust_sources(&crates, &mut files).map_err(|error| format!("cannot scan {}: {error}", crates.display()))?;
    files.sort();
    let mut issues = Vec::new();
    for file in files {
        let relative = file.strip_prefix(workspace).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        let crate_name = file
            .strip_prefix(&crates)
            .ok()
            .and_then(|relative| relative.components().nth(1))
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        let source = fs::read_to_string(&file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        if disabled_lib_tests.contains(&crate_name) {
            let file_name = file.file_name().and_then(|name| name.to_str()).unwrap_or("");
            let in_lib_src = relative.starts_with(&format!("crates/{crate_name}/src/"))
                || policy::TIERS.iter().any(|tier| relative.starts_with(&format!("crates/{tier}/{crate_name}/src/")));
            if in_lib_src && !file_name.starts_with("test_") && source.contains("#[test]") {
                issues.push(format!(
                    "{relative}: unit tests in library sources require `[lib] test` to stay enabled"
                ));
            }
        }
        issues.extend(shape_source(&crate_name, &relative, &source));
    }
    issues.sort();
    apply_shape_baseline(workspace, issues)
}

fn apply_shape_baseline(workspace: &Path, issues: Vec<String>) -> Result<Vec<String>, String> {
    // Explicit legacy exceptions are pinned to the offending source, never a whole crate or rule.
    // A multiset keeps another copy of an existing exception from silently passing.
    let mut allowed = HashMap::<(String, String, String), usize>::new();
    for row in include_str!("../shape-allowlist.tsv").lines().filter(|row| !row.starts_with('#') && !row.is_empty()) {
        let columns: Vec<_> = row.split('\t').collect();
        if columns.len() != 4 || columns[3].trim().is_empty() { return Err("invalid shape allowlist entry; each exception needs a reason".into()); }
        *allowed.entry((columns[0].into(), columns[1].into(), columns[2].into())).or_default() += 1;
    }
    let mut remaining = Vec::new();
    for issue in issues {
        let admitted = issue.split_once(": ").and_then(|(location, message)| {
            let (path, line) = location.rsplit_once(':')?;
            let line = line.parse::<usize>().ok()?;
            let source = fs::read_to_string(workspace.join(path)).ok()?;
            let snippet = source.lines().nth(line.checked_sub(1)?)?.split_whitespace().collect::<Vec<_>>().join(" ");
            let count = allowed.get_mut(&(path.into(), message.into(), snippet))?;
            if *count == 0 { return None; }
            *count -= 1;
            Some(())
        }).is_some();
        if !admitted { remaining.push(issue); }
    }
    Ok(remaining)
}

#[derive(Clone, Debug, Default)]
struct DepSpec {
    alias: String,
    package: Option<String>,
    path: Option<String>,
    workspace: bool,
    optional: bool,
    kind: String,
    target: Option<String>,
    line: usize,
}

#[derive(Clone, Debug)]
struct Package {
    manifest: PathBuf,
    package_name: String,
    crate_name: String,
    product: bool,
    /// A submodule crate (`pxui/crates/{public,private,open}/*`): in the graph so reach rules see through it, but policed by its own repository's gate.
    external: bool,
    deps: Vec<DepSpec>,
    has_binary: bool,
    has_gpu_dev_targets: bool,
}

#[derive(Default)]
struct ParsedManifest {
    disable_auto_tests: bool,
    package_name: Option<String>,
    lib_name: Option<String>,
    static_pxui_abi: bool,
    lib_unit_tests: bool,
    has_binary: bool,
    has_gpu_dev_targets: bool,
    binary_names: Vec<String>,
    deps: Vec<DepSpec>,
    workspace_deps: Vec<DepSpec>,
}

fn without_toml_comment(line: &str) -> &str {
    let (mut quote, mut escaped) = (None, false);
    for (index, character) in line.char_indices() {
        if let Some(active) = quote {
            if active == '"' && character == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if character == active && !escaped {
                quote = None;
            }
            escaped = false;
        } else if character == '"' || character == '\'' {
            quote = Some(character);
        } else if character == '#' {
            return &line[..index];
        }
    }
    line
}

fn split_toml_path(section: &str) -> Vec<String> {
    let (mut parts, mut start, mut quote) = (Vec::new(), 0, None);
    for (index, character) in section.char_indices() {
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
        } else if character == '"' || character == '\'' {
            quote = Some(character);
        } else if character == '.' {
            parts.push(section[start..index].trim().trim_matches(|character| character == '"' || character == '\'').to_owned());
            start = index + 1;
        }
    }
    parts.push(section[start..].trim().trim_matches(|character| character == '"' || character == '\'').to_owned());
    parts
}

fn toml_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\''))) {
        Some(value[1..value.len() - 1].to_owned())
    } else {
        None
    }
}

fn assignment_fields(value: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let value = value.trim().strip_prefix('{').and_then(|value| value.strip_suffix('}')).unwrap_or(value);
    let (mut start, mut depth, mut quote, mut escaped) = (0, 0isize, None, false);
    let mut pieces = Vec::new();
    for (index, character) in value.char_indices() {
        if let Some(active) = quote {
            if active == '"' && character == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if character == active && !escaped {
                quote = None;
            }
            escaped = false;
        } else if character == '"' || character == '\'' {
            quote = Some(character);
        } else if character == '{' || character == '[' {
            depth += 1;
        } else if character == '}' || character == ']' {
            depth -= 1;
        } else if character == ',' && depth == 0 {
            pieces.push(&value[start..index]);
            start = index + 1;
        }
    }
    pieces.push(&value[start..]);
    for piece in pieces {
        if let Some((key, value)) = piece.split_once('=') {
            fields.insert(key.trim().trim_matches(|character| character == '"' || character == '\'').to_owned(), value.trim().to_owned());
        }
    }
    fields
}

fn toml_value_complete(value: &str) -> bool {
    let mut depth = 0isize;
    let (mut quote, mut escaped) = (None, false);
    for character in value.chars() {
        if let Some(active) = quote {
            if active == '"' && character == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if character == active && !escaped {
                quote = None;
            }
            escaped = false;
        } else if character == '"' || character == '\'' {
            quote = Some(character);
        } else if character == '{' || character == '[' {
            depth += 1;
        } else if character == '}' || character == ']' {
            depth -= 1;
        }
    }
    depth <= 0 && quote.is_none()
}

fn dependency(section: &[String], key: Option<&str>, value: Option<&str>, line: usize, workspace: bool) -> Option<DepSpec> {
    let dependency_sections = ["dependencies", "dev-dependencies", "build-dependencies"];
    let dep_index = section.iter().position(|part| dependency_sections.contains(&part.as_str()));
    let workspace_index = section.windows(2).position(|parts| parts[0] == "workspace" && parts[1] == "dependencies");
    let (alias, kind) = if let Some(dep_index) = dep_index {
        let alias = if let Some(key) = key {
            if section.len() != dep_index + 1 {
                return None;
            }
            key.to_owned()
        } else {
            section.get(dep_index + 1)?.clone()
        };
        (alias, section[dep_index].clone())
    } else if let Some(workspace_index) = workspace_index {
        let alias = if let Some(key) = key {
            if section.len() != workspace_index + 2 {
                return None;
            }
            key.to_owned()
        } else {
            section.get(workspace_index + 2)?.clone()
        };
        (alias, "workspace".to_owned())
    } else {
        return None;
    };
    let fields = value.map(assignment_fields).unwrap_or_default();
    Some(DepSpec {
        alias,
        package: fields.get("package").and_then(|field| toml_string(field)),
        path: fields.get("path").and_then(|field| toml_string(field)),
        workspace: workspace || fields.get("workspace").is_some_and(|field| field == "true"),
        optional: fields.get("optional").is_some_and(|field| field == "true"),
        kind,
        target: section.first().is_some_and(|part| part == "target").then(|| section[1].clone()),
        line,
    })
}

fn parse_manifest(path: &Path) -> Result<ParsedManifest, String> {
    let source = fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut sections: BTreeMap<Vec<String>, HashMap<String, (String, usize)>> = BTreeMap::new();
    let (mut has_binary, mut has_gpu_dev_targets) = (false, false);
    let mut binary_names = Vec::new();
    let (mut section, mut pending, mut pending_line) = (Vec::new(), None::<(String, String)>, 0usize);
    for (zero_line, raw_line) in source.lines().enumerate() {
        let line_number = zero_line + 1;
        let line = without_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = pending.as_mut() {
            value.push(' ');
            value.push_str(line);
            if toml_value_complete(value) {
                sections.entry(section.clone()).or_default().insert(key.clone(), (value.clone(), pending_line));
                pending = None;
            }
            continue;
        }
        if line.starts_with("[[") && line.ends_with("]]") {
            section = split_toml_path(&line[2..line.len() - 2]);
            has_binary |= section.first().is_some_and(|part| part == "bin");
            has_gpu_dev_targets |= section.first().is_some_and(|part| ["example", "bench"].contains(&part.as_str()));
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = split_toml_path(&line[1..line.len() - 1]);
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().trim_matches(|character| character == '"' || character == '\'').to_owned();
            let value = value.trim().to_owned();
            if key == "name" {
                if let Some(name) = toml_string(&value) {
                    if section == ["test"] {
                        has_gpu_dev_targets |= name.starts_with("gpu_") || name.starts_with("window_");
                    } else if section == ["bin"] {
                        binary_names.push(name);
                    }
                }
            }
            if toml_value_complete(&value) {
                sections.entry(section.clone()).or_default().insert(key, (value, line_number));
            } else {
                pending = Some((key, value));
                pending_line = line_number;
            }
        }
    }

    let field = |table: &[&str], name: &str| -> Option<String> {
        let key = table.iter().map(|part| (*part).to_owned()).collect::<Vec<_>>();
        sections.get(&key)?.get(name).and_then(|(value, _)| toml_string(value))
    };
    let lib_unit_tests = sections
        .get(&vec!["lib".into()])
        .and_then(|entries| entries.get("test"))
        .is_none_or(|(value, _)| value != "false");
    let mut parsed = ParsedManifest {
        disable_auto_tests: sections.get(&vec!["package".into()]).and_then(|entries| entries.get("autotests")).is_some_and(|(value, _)| value == "false"),
        package_name: field(&["package"], "name"),
        lib_name: field(&["lib"], "name"),
        static_pxui_abi: false,
        lib_unit_tests,
        has_binary,
        has_gpu_dev_targets,
        binary_names,
        ..ParsedManifest::default()
    };
    for (section, entries) in &sections {
        if section.len() >= 2 && section[0] == "workspace" && section[1] == "dependencies" {
            if section.len() == 2 {
                for (alias, (value, line)) in entries {
                    if let Some(dependency) = dependency(section, Some(alias), Some(value), *line, false) {
                        parsed.workspace_deps.push(dependency);
                    }
                }
            } else if let Some(mut dependency) = dependency(section, None, Some("{}"), entries.values().map(|(_, line)| *line).min().unwrap_or(1), false) {
                let fields: HashMap<String, String> = entries.iter().map(|(key, (value, _))| (key.clone(), value.clone())).collect();
                dependency.package = fields.get("package").and_then(|field| toml_string(field));
                dependency.path = fields.get("path").and_then(|field| toml_string(field));
                dependency.optional = fields.get("optional").is_some_and(|field| field == "true");
                parsed.workspace_deps.push(dependency);
            }
            continue;
        }
        if section.iter().any(|part| ["dependencies", "dev-dependencies", "build-dependencies"].contains(&part.as_str())) {
            if section.last().is_some_and(|part| ["dependencies", "dev-dependencies", "build-dependencies"].contains(&part.as_str())) {
                for (alias, (value, line)) in entries {
                    if let Some(dependency) = dependency(section, Some(alias), Some(value), *line, false) {
                        parsed.deps.push(dependency);
                    }
                }
            } else if let Some(mut dependency) = dependency(section, None, Some("{}"), entries.values().map(|(_, line)| *line).min().unwrap_or(1), false) {
                let fields: HashMap<String, String> = entries.iter().map(|(key, (value, _))| (key.clone(), value.clone())).collect();
                dependency.package = fields.get("package").and_then(|field| toml_string(field));
                dependency.path = fields.get("path").and_then(|field| toml_string(field));
                dependency.workspace = fields.get("workspace").is_some_and(|field| field == "true");
                dependency.optional = fields.get("optional").is_some_and(|field| field == "true");
                parsed.deps.push(dependency);
            }
        }
    }
    parsed.static_pxui_abi = sections.get(&vec!["features".into()]).and_then(|entries| entries.get("static"))
        .is_some_and(|(value, _)| feature_reaches_package(value, "pxui_abi", &parsed.deps));
    Ok(parsed)
}

fn feature_reaches_package(value: &str, package: &str, deps: &[DepSpec]) -> bool {
    let references: HashSet<_> = value.split(|character: char| !character.is_ascii_alphanumeric() && character != '_').collect();
    deps.iter().any(|dep| {
        dep.optional
            && dep.package.as_deref().unwrap_or(&dep.alias) == package
            && references.contains(dep.alias.as_str())
    })
}

fn cargo_manifests(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|error| format!("cannot scan {}: {error}", dir.display()))? {
        let entry = entry.map_err(|error| format!("cannot scan {}: {error}", dir.display()))?;
        let path = entry.path();
        let manifest = path.join("Cargo.toml");
        if manifest.is_file() {
            files.push(manifest);
        } else if entry.file_type().map_err(|error| format!("cannot inspect {}: {error}", path.display()))?.is_dir() {
            cargo_manifests(&path, files)?;
        }
    }
    Ok(())
}

fn tiers(workspace: &Path) -> Result<Vec<String>, String> {
    let workspace = fs::canonicalize(workspace).map_err(|error| format!("cannot locate workspace {}: {error}", workspace.display()))?;
    let workspace_deps: HashMap<String, DepSpec> = parse_manifest(&workspace.join("Cargo.toml"))?.workspace_deps
        .into_iter().map(|dependency| (dependency.alias.clone(), dependency)).collect();
    let mut manifests = Vec::new();
    cargo_manifests(&workspace.join("crates"), &mut manifests)?;
    manifests.sort();
    let mut packages = Vec::new();
    let mut by_manifest = HashMap::new();
    let mut errors = Vec::new();
    for manifest in manifests {
        let manifest = fs::canonicalize(&manifest).map_err(|error| format!("cannot resolve {}: {error}", manifest.display()))?;
        let relative = manifest.strip_prefix(&workspace).unwrap_or(&manifest).to_string_lossy().replace('\\', "/");
        let parts: Vec<_> = relative.split('/').collect();
        if parts.len() != 4 || parts[0] != "crates" || !policy::TIERS.contains(&parts[1]) || parts[3] != "Cargo.toml" {
            errors.push(format!("{relative}: crate must live under crates/public, crates/private or crates/open"));
            continue;
        }
        let parsed = parse_manifest(&manifest)?;
        let Some(package_name) = parsed.package_name.as_deref() else { continue };
        let crate_name = crate_name(package_name, parsed.lib_name.as_deref());
        if parts[2] != package_name.replace('-', "_") {
            errors.push(format!("{relative}: tier folder must match package `{package_name}`"));
            continue;
        }
        by_manifest.insert(manifest.clone(), (parts[1].to_owned(), crate_name.clone()));
        packages.push((manifest, parsed, parts[1].to_owned(), crate_name));
    }
    for (manifest, parsed, from_tier, from_crate) in packages {
        let parent = manifest.parent().unwrap_or(&workspace);
        for mut dependency in parsed.deps {
            // Test-only edges do not ship from the crate and may exercise adjacent private tiers.
            if dependency.kind == "dev-dependencies" { continue; }
            if dependency.workspace {
                if let Some(inherited) = workspace_deps.get(&dependency.alias) {
                    if dependency.package.is_none() { dependency.package = inherited.package.clone(); }
                    if dependency.path.is_none() { dependency.path = inherited.path.clone(); }
                }
            }
            let Some(path) = dependency.path.as_deref() else { continue };
            let target_manifest = match fs::canonicalize(parent.join(path).join("Cargo.toml")) {
                Ok(path) => path,
                Err(error) => {
                    errors.push(format!("{}:{}: cannot resolve tier dependency `{}`: {error}", manifest.display(), dependency.line, dependency.alias));
                    continue;
                }
            };
            let Some((to_tier, to_crate)) = by_manifest.get(&target_manifest) else {
                errors.push(format!("{}:{}: `{from_crate}` may not depend on non-tier crate `{}`", manifest.display(), dependency.line, dependency.package.as_deref().unwrap_or(&dependency.alias)));
                continue;
            };
            let dependency_crate = dependency.package.as_deref().unwrap_or(&dependency.alias).replace('-', "_");
            if !policy::tier_allows(&from_tier, &from_crate, to_tier, &dependency_crate, dependency.optional, parsed.static_pxui_abi) {
                errors.push(format!("{}:{}: {from_tier} crate `{from_crate}` may not depend on {to_tier} crate `{to_crate}`", manifest.display(), dependency.line));
            }
        }
    }
    errors.sort();
    Ok(errors)
}

fn package_targets(dir: &Path, manifest: &ParsedManifest) -> Result<(bool, bool), String> {
    let mut files = Vec::new();
    rust_sources(dir, &mut files).map_err(|error| format!("cannot scan {}: {error}", dir.display()))?;
    // print_shader only writes generated source/SPIR-V; ogpu_core supplies shader metadata.
    // Any additional executable in this crate still needs the ordinary startup check.
    let shader_printer = manifest.package_name.as_deref() == Some("flow_shader")
        && manifest.binary_names == ["print_shader"];
    let mut has_binary = (manifest.has_binary && !shader_printer) || dir.join("src/main.rs").is_file();
    let mut has_gpu_dev_targets = manifest.has_gpu_dev_targets;
    for file in files {
        let relative = file.strip_prefix(dir).unwrap_or(&file);
        let mut components = relative.components();
        let first = components.next().and_then(|part| part.as_os_str().to_str());
        let second = components.next().and_then(|part| part.as_os_str().to_str());
        has_binary |= first == Some("src") && second == Some("bin");
        // Manifest regeneration evaluates Lisp and writes a text fixture; it never starts a GPU.
        let manifest_writer = manifest.package_name.as_deref() == Some("web_export")
            && relative == Path::new("examples/refresh_manifest.rs");
        has_gpu_dev_targets |= matches!(first, Some("examples" | "benches")) && !manifest_writer;
        // Same native target convention as rays-t; ordinary unit/e2e tests may use GPU types
        // without creating a device. Explicit targets are recorded while parsing the manifest.
        has_gpu_dev_targets |= !manifest.disable_auto_tests && first == Some("tests") && components.next().is_none()
            && file.file_stem().and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("gpu_") || name.starts_with("window_"));
    }
    Ok((has_binary, has_gpu_dev_targets))
}

fn crate_name(package_name: &str, lib_name: Option<&str>) -> String {
    lib_name.unwrap_or(package_name).replace('-', "_")
}

fn graph_crate_name(package: &str, crate_name: &str) -> String {
    if package == "pxui_abi" && crate_name == "pxui" {
        "pxui_abi".to_owned()
    } else {
        crate_name.to_owned()
    }
}

mod policy;
use policy::{direct_allowlist, forbidden_reaches, local_workspace_only, HELPER_LEAVES, OCAML_LEAVES};

/// OCaml leaves that `start` reaches through its allowed dependencies (transitively, over the policy).
fn ocaml_leaves_reached(start: &str, allowlist: &HashMap<&'static str, &'static [&'static str]>) -> HashSet<&'static str> {
    let mut reached = HashSet::new();
    let mut seen = HashSet::new();
    let mut pending: Vec<&'static str> = allowlist.get(start).map(|allowed| allowed.to_vec()).unwrap_or_default();
    while let Some(name) = pending.pop() {
        if !seen.insert(name) {
            continue;
        }
        if OCAML_LEAVES.contains(&name) {
            reached.insert(name);
        }
        if let Some(next) = allowlist.get(name) {
            pending.extend(next.iter().copied());
        }
    }
    reached
}

fn dependency_path(start: &str, goal: &str, graph: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut pending = vec![(start.to_owned(), vec![start.to_owned()])];
    let mut seen = HashSet::new();
    while let Some((current, path)) = pending.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if current == goal {
            return Some(path);
        }
        if let Some(next) = graph.get(&current) {
            for dependency in next.iter().rev() {
                let mut path = path.clone();
                path.push(dependency.clone());
                pending.push((dependency.clone(), path));
            }
        }
    }
    None
}
fn target_platform(target: &str) -> Option<&'static str> {
    let target = target.to_ascii_lowercase();
    if target.contains("not") || target.contains('!') {
        return None;
    }
    let apple = target.contains("apple") || target.contains("macos") || target.contains("ios");
    let windows = target.contains("windows");
    // A Linux-only target (the Wayland host's tests) selects nothing on the two gated platforms.
    let linux = target.contains("linux");
    match (apple, windows) {
        (true, false) => Some("apple"),
        (false, true) => Some("windows"),
        (false, false) if linux => Some("linux"),
        _ => None,
    }
}

fn ogpu_selection_errors(
    packages: &[Package],
    graph: &HashMap<String, Vec<String>>,
    direct: &HashMap<String, Vec<String>>,
    workspace: &Path,
) -> Vec<String> {
    let mut errors = Vec::new();
    for package in packages.iter().filter(|package| !package.external) {
        let all_selected = direct.get(&package.crate_name).map(Vec::as_slice).unwrap_or_default();
        let path = package.manifest.strip_prefix(workspace).unwrap_or(&package.manifest).to_string_lossy().replace('\\', "/");
        // Production startup cannot be satisfied by a dev dependency. Tests may explicitly
        // select mock in one target and the platform-native implementation in another.
        for tests in [false, true] {
            if !(if tests { package.has_gpu_dev_targets } else { package.has_binary }) {
                continue;
            }
            let declarations: Vec<_> = package.deps.iter().filter(|dependency| {
                dependency.kind == "dependencies" || (tests && dependency.kind == "dev-dependencies")
            }).collect();
            for dependency in &declarations {
                let name = dependency.package.as_deref().unwrap_or(&dependency.alias).replace('-', "_");
                let expected = match name.as_str() {
                    "ogpu_metal" => "apple",
                    "ogpu_vulkan" => "windows",
                    _ => continue,
                };
                let platform = dependency.target.as_deref().and_then(target_platform);
                if dependency.optional || !(platform == Some(expected) || (expected == "windows" && platform == Some("linux"))) {
                    errors.push(format!("{path}: {} backend {} must be nonoptional and target {expected}", package.crate_name, dependency.alias));
                }
            }
            for (platform, native) in [("apple", "ogpu_metal"), ("windows", "ogpu_vulkan")] {
                // An implementation's native tests run only on its own platform.
                if tests && matches!(package.crate_name.as_str(), "ogpu_metal" | "ogpu_vulkan")
                    && package.crate_name != native
                {
                    continue;
                }
                let selected: Vec<_> = all_selected.iter().filter(|name| {
                    declarations.iter().any(|dependency| {
                        dependency.package.as_deref().unwrap_or(&dependency.alias).replace('-', "_") == **name
                            && !dependency.optional
                            && dependency.target.as_deref().and_then(target_platform).is_none_or(|target| target == platform)
                    })
                }).map(String::as_str).collect();
                let reaches_ogpu = selected.iter().any(|dependency| {
                    *dependency == "ogpu_core" || dependency_path(dependency, "ogpu_core", graph).is_some()
                });
                if !reaches_ogpu {
                    continue;
                }
                let native_count = declarations.iter().filter(|dependency| {
                    dependency.package.as_deref().unwrap_or(&dependency.alias).replace('-', "_") == native
                        && !dependency.optional
                        && dependency.target.as_deref().and_then(target_platform) == Some(platform)
                }).count() + usize::from(tests && package.crate_name == native);
                let mock = selected.contains(&"ogpu_mock") || (tests && package.crate_name == "ogpu_mock");
                if native_count <= 1 && (native_count == 1 || (tests && mock)) && (tests || !mock) {
                    continue;
                }
                let role = if tests { "GPU/window tests or dev executable" } else { "native executable" };
                errors.push(format!(
                    "{path}: {role} {} must select one nonoptional {native} {}on {platform}{}",
                    package.crate_name,
                    if tests { "" } else { "normal dependency " },
                    if tests { " (mock may also be selected)" } else { "" },
                ));
            }
        }
    }
    errors
}

fn deps(workspace: &Path) -> Result<Vec<String>, String> {
    let workspace_path = fs::canonicalize(workspace).map_err(|error| format!("cannot locate workspace {}: {error}", workspace.display()))?;
    let workspace = workspace_path.as_path();
    let root_manifest = parse_manifest(&workspace.join("Cargo.toml"))?;
    let workspace_deps: HashMap<String, DepSpec> = root_manifest.workspace_deps.into_iter().map(|dependency| (dependency.alias.clone(), dependency)).collect();
    let mut manifests = Vec::new();
    cargo_manifests(&workspace.join("crates"), &mut manifests)?;
    cargo_manifests(&workspace.join("tools"), &mut manifests)?;
    cargo_manifests(&workspace.join("bin"), &mut manifests)?;
    cargo_manifests(&workspace.join("pxui/crates"), &mut manifests)?;
    manifests.sort();

    let mut packages = Vec::new();
    let mut errors = Vec::new();
    for manifest_path in manifests {
        let manifest_path = fs::canonicalize(&manifest_path).map_err(|error| format!("cannot resolve {}: {error}", manifest_path.display()))?;
        let parsed = parse_manifest(&manifest_path)?;
        let Some(package_name) = parsed.package_name.as_deref() else { continue };
        let relative = manifest_path.strip_prefix(workspace).unwrap_or(&manifest_path).to_string_lossy().replace('\\', "/");
        let external = relative.starts_with("pxui/");
        let is_product = relative.starts_with("crates/") || relative.starts_with("bin/") || external;
        let crate_name = crate_name(package_name, parsed.lib_name.as_deref());
        let (has_binary, has_gpu_dev_targets) = package_targets(manifest_path.parent().unwrap_or(workspace), &parsed)?;
        let mut dependencies = Vec::new();
        for mut dependency in parsed.deps {
            if dependency.workspace {
                if let Some(inherited) = workspace_deps.get(&dependency.alias) {
                    if dependency.package.is_none() { dependency.package = inherited.package.clone(); }
                    if dependency.path.is_none() {
                        dependency.path = inherited.path.as_ref().map(|path| workspace.join(path).to_string_lossy().into_owned());
                    }
                    dependency.workspace = inherited.workspace;
                }
            }
            dependencies.push(dependency);
        }
        packages.push(Package {
            manifest: manifest_path,
            package_name: package_name.to_owned(),
            crate_name,
            product: is_product,
            external,
            deps: dependencies,
            has_binary,
            has_gpu_dev_targets,
        });
    }

    let mut by_manifest = HashMap::new();
    let mut by_package = HashMap::new();
    let mut by_crate = HashMap::new();
    for (index, package) in packages.iter().enumerate() {
        by_manifest.insert(package.manifest.clone(), index);
        if by_package.insert(package.package_name.clone(), index).is_some() {
            errors.push(format!("{}: duplicate workspace package `{}`", package.manifest.display(), package.package_name));
        }
        let graph_name = graph_crate_name(&package.package_name, &package.crate_name);
        if by_crate.insert(graph_name.clone(), index).is_some() {
            errors.push(format!("{}: duplicate crate identity `{graph_name}`", package.manifest.display()));
        }
    }
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut runtime_graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut selection_direct: HashMap<String, Vec<String>> = HashMap::new();
    let allowlist = direct_allowlist();
    let package_allowlist = policy::package_allowlist();
    let local_policy = local_workspace_only();

    for package in &packages {
        let errors_before = errors.len();
        let manifest_dir = package.manifest.parent().unwrap_or(workspace);
        let path = package.manifest.strip_prefix(workspace).unwrap_or(&package.manifest).to_string_lossy().replace('\\', "/");
        let allowed = package_allowlist.get(package.package_name.as_str()).or_else(|| allowlist.get(package.crate_name.as_str()));
        if package.product
            && allowed.is_none()
            && !local_policy.contains(package.crate_name.as_str())
        {
            errors.push(format!("{path}: no dependency policy for product crate `{}`", package.crate_name));
        }
        let mut direct = BTreeSet::new();
        let mut graph_direct = BTreeSet::new();
        let mut selection = BTreeSet::new();
        let mut runtime_direct = BTreeSet::new();
        for dependency in &package.deps {
            let dependency_package = dependency.package.as_deref().unwrap_or(&dependency.alias);
            let resolved = if let Some(local_path) = &dependency.path {
                let dep_manifest = manifest_dir.join(local_path).join("Cargo.toml");
                let dep_manifest = fs::canonicalize(&dep_manifest).unwrap_or(dep_manifest);
                if !dep_manifest.starts_with(workspace) {
                    errors.push(format!("{path}:{}: dependency `{}` is not a workspace crate", dependency.line, dependency.alias));
                    continue;
                }
                match by_manifest.get(&dep_manifest).and_then(|index| packages.get(*index)) {
                    Some(target) if target.package_name == dependency_package => Some(target),
                    Some(target) => {
                        errors.push(format!("{path}:{}: dependency `{}` names package `{dependency_package}` but its path is `{}`", dependency.line, dependency.alias, target.package_name));
                        Some(target)
                    }
                    None => {
                        errors.push(format!("{path}:{}: dependency `{}` does not resolve to a workspace crate", dependency.line, dependency.alias));
                        None
                    }
                }
            } else if (dependency_package == "cc" && dependency.kind == "build-dependencies")
                || (dependency.kind == "dependencies" && policy::EXTERNAL_CRATES.contains(&(package.package_name.as_str(), dependency_package)))
            {
                None
            } else {
                errors.push(format!("{path}:{}: external dependency `{dependency_package}` is not allowed; use a local workspace crate (cc is the sole exception)", dependency.line));
                None
            };
            if let Some(target) = resolved {
                if dependency.kind != "dev-dependencies"
                    && policy::NO_DIRECT.iter().any(|(from, to, _)| *from == package.crate_name && *to == target.crate_name)
                {
                    let why = policy::NO_DIRECT.iter().find(|(from, to, _)| *from == package.crate_name && *to == target.crate_name).map_or("", |row| row.2);
                    errors.push(format!("{path}:{}: {} depends directly on {} ({why})", dependency.line, package.crate_name, target.crate_name));
                }
                if dependency.kind != "dev-dependencies" {
                    direct.insert(target.crate_name.clone());
                    graph_direct.insert(graph_crate_name(&target.package_name, &target.crate_name));
                }
                if dependency.kind != "build-dependencies" {
                    selection.insert(graph_crate_name(&target.package_name, &target.crate_name));
                }
                if dependency.kind == "dependencies" {
                    runtime_direct.insert(graph_crate_name(&target.package_name, &target.crate_name));
                }
                if package.product && !target.product {
                    errors.push(format!("{path}:{}: product crate `{}` must not depend on tool crate `{}`", dependency.line, package.crate_name, target.crate_name));
                }
            }
        }
        let direct: Vec<String> = direct.into_iter().collect();
        let source_graph_name = graph_crate_name(&package.package_name, &package.crate_name);
        graph.insert(source_graph_name.clone(), graph_direct.into_iter().collect());
        let runtime_direct: Vec<String> = runtime_direct.into_iter().collect();
        runtime_graph.insert(source_graph_name.clone(), runtime_direct);
        let selection: Vec<String> = selection.into_iter().collect();
        selection_direct.insert(source_graph_name, selection);
        if let Some(allowed) = allowed {
            let leaves = ocaml_leaves_reached(&package.crate_name, &allowlist);
            for dependency in &direct {
                let name = dependency.as_str();
                if !allowed.contains(&name) && !HELPER_LEAVES.contains(&name) && !leaves.contains(name) {
                    errors.push(format!("{path}: `{}` may not directly depend on `{dependency}`", package.crate_name));
                }
            }
        }
        if package.external { errors.truncate(errors_before); }
    }

    let reach_rules = forbidden_reaches();
    for (library, forbidden) in reach_rules {
        if !by_crate.get(&library).is_some_and(|index| !packages[*index].external) {
            continue;
        }
        for target in forbidden {
            if target == library {
                continue;
            }
            if let Some(path) = dependency_path(&library, &target, &graph) {
                errors.push(format!("dependency boundary: {} must not reach `{target}` (via {})", library, path.join(" -> ")));
            }
        }
    }
    // The PXUI crates build and test with no SDL and nothing from Rays: their tests (dev
    // dependencies) may not reach the window host, SDL or any Rays crate either.
    for library in policy::TESTS_INCLUDED {
        if !by_crate.get(*library).is_some_and(|index| !packages[*index].external) { continue; }
        for dependency in selection_direct.get(*library).into_iter().flatten() {
            for target in policy::TESTS_INCLUDED_FORBIDDEN.iter().copied() {
                let path = if dependency == target { Some(vec![dependency.clone()]) } else { dependency_path(dependency, target, &graph) };
                if let Some(path) = path {
                    errors.push(format!("dependency boundary: {library} (tests included) must not reach `{target}` (via {library} -> {})", path.join(" -> ")));
                }
            }
        }
    }
    errors.extend(ogpu_selection_errors(&packages, &runtime_graph, &selection_direct, workspace));
    errors.sort();
    Ok(errors)
}


/// With a `pxui/` checkout: its recorded commit must be on the PXUI remote (tools/check-pxui-remote.sh).
fn pxui_remote(workspace: &Path) -> Result<Vec<String>, String> {
    if !workspace.join("pxui").exists() {
        return Ok(Vec::new());
    }
    let output = std::process::Command::new("sh").arg("tools/check-pxui-remote.sh").current_dir(workspace).output()
        .map_err(|error| format!("cannot run tools/check-pxui-remote.sh: {error}"))?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(if output.status.success() { Vec::new() } else { vec![String::from_utf8_lossy(&output.stderr).trim().to_owned()] })
}

fn install_smoke(workspace: &Path) -> Result<Vec<String>, String> {
    if cfg!(windows) {
        println!("install-smoke: skipped on Windows (tools/install.sh installs the macOS and Linux libraries only)");
        return Ok(Vec::new());
    }
    let script = workspace.join("tools/install-smoke.sh");
    let status = std::process::Command::new(&script)
        .current_dir(workspace)
        .status()
        .map_err(|error| format!("cannot run {}: {error}", script.display()))?;
    Ok(if status.success() { Vec::new() } else { vec![format!("install-smoke: {} failed ({status})", script.display())] })
}

fn docs(workspace: &Path) -> Result<Vec<String>, String> {
    // rays-t bounds setup/build, each case and the test run. Gate checks never refresh fixtures.
    for (package, filter, target) in policy::DOCS_CHECKS.iter().copied() {
        let mut command = std::process::Command::new("cargo");
        command.current_dir(workspace).args(["t", "--profile", "default", "-p", package])
            .env_remove("RAYS_UPDATE_FIXTURES").env_remove("RAYS_ALLOW_SKIP");
        if let Some(target) = target { command.args(["--test", target]); }
        let status = command.arg(filter).status().map_err(|error| format!("cannot run registry check: {error}"))?;
        if !status.success() {
            return Ok(vec![format!("docs gate: {package}::{filter} failed ({status}); examples, descriptions, angle units and docs/lisp.md must match the registry")]);
        }
    }
    Ok(Vec::new())
}

fn dynamic_symbols(os: &str, output: &str) -> Vec<String> {
    if os == "windows" {
        let (mut symbols, mut gnu_table, mut llvm_table, mut saw_llvm_name) = (Vec::new(), false, false, false);
        for line in output.lines() {
            if line.contains("[Ordinal/Name Pointer] Table") {
                gnu_table = true;
                continue;
            }
            if line.trim() == "Export Table:" {
                llvm_table = true;
                continue;
            }
            if gnu_table {
                if let Some((_, rest)) = line.split_once(']') {
                    if let Some(name) = rest.split_whitespace().last() { symbols.push(name.to_owned()); }
                } else if !symbols.is_empty() && !line.trim().is_empty() {
                    break;
                }
            } else if llvm_table {
                let mut fields = line.split_whitespace();
                if fields.next().is_some_and(|field| field.parse::<u32>().is_ok()) {
                    if let Some(name) = fields.last() { symbols.push(name.to_owned()); }
                    saw_llvm_name = true;
                } else if saw_llvm_name && line.trim().is_empty() {
                    break;
                }
            }
        }
        symbols
    } else {
        output.lines()
            .filter_map(|line| line.split_whitespace().last())
            .map(|name| name.trim_start_matches('_').to_owned())
            .collect()
    }
}

fn abi_exports(workspace: &Path) -> Result<Vec<String>, String> {
    let status = std::process::Command::new("cargo")
        .current_dir(workspace)
        .args(["build", "-p", "pxui_abi"])
        .status()
        .map_err(|error| format!("cannot build pxui_abi: {error}"))?;
    if !status.success() {
        return Ok(vec![format!("abi-exports: cargo build -p pxui_abi failed ({status})")]);
    }

    let mut target = env::var_os("CARGO_TARGET_DIR").map_or_else(|| workspace.join("target"), |path| {
        let path = PathBuf::from(path);
        if path.is_absolute() { path } else { workspace.join(path) }
    });
    if let Some(triple) = env::var_os("CARGO_BUILD_TARGET") {
        target.push(triple);
    }
    let library = target.join("debug").join(format!("{}pxui{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX));
    let (tool, args): (&str, &[&str]) = match env::consts::OS {
        "macos" => ("nm", &["-gU"]),
        "linux" => ("nm", &["-D", "--defined-only"]),
        "windows" => ("objdump", &["-p"]),
        os => return Err(format!("abi-exports: unsupported symbol-table tool for {os}")),
    };
    let output = std::process::Command::new(tool)
        .args(args)
        .arg(&library)
        .output()
        .map_err(|error| format!("cannot list exports from {} with {tool}: {error}", library.display()))?;
    if !output.status.success() {
        return Err(format!("{tool} failed for {}: {}", library.display(), String::from_utf8_lossy(&output.stderr).trim()));
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let symbols = dynamic_symbols(env::consts::OS, &output);
    let mut issues: Vec<_> = symbols.iter().filter(|name| !name.starts_with("pxui_")).map(|name| format!("abi-exports: unexpected dynamic symbol {name}")).collect();
    if !symbols.iter().any(|name| name == "pxui_abi_version") {
        issues.push("abi-exports: missing pxui_abi_version".to_owned());
    }
    Ok(issues)
}

fn abi_minor(source: &str) -> Result<u32, String> {
    let declaration = source
        .lines()
        .find(|line| line.trim_start().starts_with("pub const ABI_VERSION:"))
        .ok_or_else(|| "abi-snapshot: missing ABI_VERSION".to_owned())?;
    let value = declaration
        .split_once('=')
        .and_then(|(_, value)| value.split(';').next())
        .ok_or_else(|| "abi-snapshot: malformed ABI_VERSION".to_owned())?
        .trim();
    let version = if let Some((high, shift)) = value.split_once("<<") {
        high.trim().parse::<u32>().ok().and_then(|high| shift.trim().parse::<u32>().ok().map(|shift| high << shift))
    } else {
        value.trim_start_matches("0x").parse::<u32>().ok()
    }.ok_or_else(|| format!("abi-snapshot: unsupported ABI_VERSION expression `{value}`"))?;
    Ok(version & 0xffff)
}

fn normalized_declaration(source: &str) -> String {
    let tokens = lex(source);
    tokens.into_iter().map(|token| token.text).collect::<Vec<_>>().join(" ")
}

fn c_exports(header: &str) -> BTreeMap<String, String> {
    let mut declarations = BTreeMap::new();
    let mut clean = String::with_capacity(header.len());
    let mut chars = header.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') { chars.next(); break; }
            }
            clean.push(' ');
        } else if ch == '/' && chars.peek() == Some(&'/') {
            while chars.next().is_some_and(|ch| ch != '\n') {}
            clean.push(' ');
        } else {
            clean.push(ch);
        }
    }
    for declaration in clean.split(';') {
        let Some(paren) = declaration.find('(') else { continue };
        let Some(name) = declaration[..paren].split_whitespace().last() else { continue };
        let name = name.trim_start_matches('*');
        if !name.starts_with("pxui_") { continue; }
        declarations.insert(name.to_owned(), normalized_declaration(&format!("{};", declaration.trim())));
    }
    declarations
}

// Header names carry a `Pxui` prefix the Rust names drop; a few Rust names differ on purpose.
// ponytail: every `*Span` is one `Slice`, so its element type is not compared here (layout rows pin the structs); compare it when a Span mix-up ships.
fn canonical_name(name: &str) -> &str {
    let name = name.strip_prefix("Pxui").filter(|rest| !rest.is_empty() && name != "PxuiUi").unwrap_or(name);
    if name.ends_with("Span") || name == "Slice" { return "Slice"; }
    match name {
        "Color" => "Rgba",
        "FfiEvent" => "Event",
        "FfiOutput" => "Output",
        "uint16_t" | "u16" => "u16",
        "PxuiStr" | "Str" => "Str",
        "PxuiFrameIn" | "FrameIn" => "FrameIn",
        "PxuiFrameOut" | "FrameOut" => "FrameOut",
        "PxuiGpu" | "Gpu" => "Gpu",
        "PxuiCommands" | "Commands" => "Commands",
        "PxuiPass" | "Pass" => "Pass",
        "PxuiImage" | "Image" => "Image",
        "PxuiText" | "Text" => "Text",
        "PxuiTextStyle" | "TextStyle" => "TextStyle",
        "PxuiTextMetrics" | "TextMetrics" => "TextMetrics",
        "PxuiTextGlyph" | "TextGlyph" => "TextGlyph",
        "PxuiTargetDesc" => "TargetDesc",
        "PxuiPanel" => "Panel",
        "PxuiBox" => "Box",
        "PxuiBuild" | "Build" => "Build",
        "void" => "c_void",
        "char" | "c_char" => "char",
        "uint8_t" | "u8" => "u8",
        "uint32_t" | "u32" => "u32",
        "uint64_t" | "u64" => "u64",
        "int32_t" | "i32" => "i32",
        "int64_t" | "i64" => "i64",
        "size_t" | "usize" => "usize",
        "float" | "f32" => "f32",
        "double" | "f64" => "f64",
        other => other,
    }
}

fn canonical_type(tokens: &[Token], rust: bool) -> String {
    let text: Vec<_> = tokens.iter().map(|token| token.text.as_str()).collect();
    if rust && text.first() == Some(&"*") {
        let mut at = 0;
        let mut pointers = Vec::new();
        while text.get(at) == Some(&"*") {
            pointers.push(text.get(at + 1).copied().unwrap_or("mut"));
            at += 2;
        }
        let target = text.get(at..).unwrap_or(&[]);
        let name = target.iter().rev().find(|&&token| token != ":" && token != "*").copied().unwrap_or("");
        let mut canonical = canonical_name(name).to_owned();
        for qualifier in pointers.iter().rev() { canonical = format!("*{qualifier} {canonical}"); }
        return canonical;
    }
    if !rust {
        if let Some(pointer) = text.iter().position(|&token| token == "*") {
            let depth = text[pointer..].iter().filter(|&&token| token == "*").count();
            let is_const = text[..pointer].contains(&"const");
            let name = text[..pointer].iter().rev().find(|&&token| token != "const" && token != "volatile").copied().unwrap_or("");
            let mut canonical = canonical_name(name).to_owned();
            for index in 0..depth {
                let qualifier = if index == 0 && is_const { "const" } else { "mut" };
                canonical = format!("*{qualifier} {canonical}");
            }
            return canonical;
        }
    }
    if text.is_empty() || text == ["void"] { return "()".to_owned(); }
    let text = if rust { &text[..text.iter().position(|&token| token == "<").unwrap_or(text.len())] } else { &text[..] };
    let name = if rust && text.len() >= 3 && text[1] == ":" && text[2] == ":" {
        text.last().copied().unwrap_or("")
    } else { text.last().copied().unwrap_or("") };
    canonical_name(name).to_owned()
}

fn c_signature(declaration: &str) -> String {
    let tokens = lex(declaration);
    let Some(open) = tokens.iter().position(|token| token.text == "(") else { return String::new() };
    let Some(close) = matching(&tokens, open, "(", ")") else { return String::new() };
    let result = canonical_type(&tokens[..open - 1], false);
    let mut params = Vec::new();
    let mut start = open + 1;
    let mut depth = 0usize;
    for index in open + 1..=close {
        if index == close || (tokens[index].text == "," && depth == 0) {
            let part = &tokens[start..index];
            if !(part.len() == 1 && part[0].text == "void") && !part.is_empty() {
                let array = part.iter().position(|token| token.text == "[");
                let declarator = array.map_or(part, |at| &part[..at]);
                let ty = &declarator[..declarator.len().saturating_sub(1)];
                let mut canonical = canonical_type(ty, false);
                if array.is_some() {
                    if canonical.starts_with('*') {
                        canonical = format!("*mut {canonical}");
                    } else {
                        let qualifier = if ty.iter().any(|token| token.text == "const") { "const" } else { "mut" };
                        canonical = format!("*{qualifier} {canonical}");
                    }
                }
                params.push(canonical);
            }
            start = index + 1;
        } else if tokens[index].text == "(" { depth += 1; }
        else if tokens[index].text == ")" { depth = depth.saturating_sub(1); }
    }
    format!("({})->{result}", params.join(","))
}

fn rust_signature(tokens: &[Token], fn_at: usize) -> Option<(String, String)> {
    let name = tokens.get(fn_at + 1)?.text.clone();
    let open = fn_at + 2;
    let close = matching(tokens, open, "(", ")")?;
    let mut params = Vec::new();
    let mut start = open + 1;
    let mut depth = 0usize;
    for index in open + 1..=close {
        if index == close || (tokens[index].text == "," && depth == 0) {
            let part = &tokens[start..index];
            if !part.is_empty() {
                let colon = part.iter().position(|token| token.text == ":")?;
                params.push(canonical_type(&part[colon + 1..], true));
            }
            start = index + 1;
        } else if matches!(tokens[index].text.as_str(), "<" | "(" | "[") { depth += 1; }
        else if matches!(tokens[index].text.as_str(), ">" | ")" | "]") { depth = depth.saturating_sub(1); }
    }
    let tail = &tokens[close + 1..];
    // The return type ends at the body; a function without `->` must not borrow the next one's.
    let body = tail.iter().position(|token| token.text == "{" || token.text == ";").unwrap_or(tail.len());
    let result = tail[..body].iter().position(|token| token.text == "->")
        .map(|arrow| canonical_type(&tail[arrow + 1..body], true))
        .unwrap_or_else(|| "()".to_owned());
    Some((name, format!("({})->{result}", params.join(","))))
}

fn rust_exports(source: &str) -> BTreeMap<String, String> {
    let tokens = lex(source);
    let mut declarations = BTreeMap::new();
    for (at, token) in tokens.iter().enumerate() {
        if token.text != "fn" { continue; }
        let Some(name) = tokens.get(at + 1).map(|token| token.text.as_str()) else { continue };
        if !name.starts_with("pxui_") { continue; }
        let start = (0..at).rev().find(|&index| matches!(tokens[index].text.as_str(), "}" | ";")).map_or(0, |index| index + 1);
        let prefix = tokens[start..at].iter().map(|token| token.text.as_str()).collect::<Vec<_>>();
        let macro_export = prefix.windows(4).any(|window| window == ["export", "!", "(", "pub"])
            || prefix.windows(3).any(|window| window == ["export", "!", "("]);
        let no_mangle = prefix.iter().any(|token| *token == "no_mangle");
        if !macro_export && !no_mangle { continue; }
        if let Some((name, signature)) = rust_signature(&tokens, at) {
            declarations.insert(name, signature);
        }
    }
    declarations
}

fn abi_declarations(workspace: &Path) -> Result<(u32, BTreeMap<String, String>, BTreeMap<String, String>), String> {
    let header_path = workspace.join("include/pxui.h");
    let header = fs::read_to_string(&header_path).map_err(|error| format!("{}: {error}", header_path.display()))?;
    let source_dir = workspace.join("crates/private/pxui_abi/src");
    let source_path = source_dir.join("lib.rs");
    let source = fs::read_to_string(&source_path).map_err(|error| format!("{}: {error}", source_path.display()))?;
    let c = c_exports(&header);
    let mut source_files = Vec::new();
    rust_sources(&source_dir, &mut source_files).map_err(|error| format!("cannot scan {}: {error}", source_dir.display()))?;
    source_files.sort();
    let mut rust = BTreeMap::new();
    for path in source_files {
        let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        rust.extend(rust_exports(&text));
    }
    Ok((abi_minor(&source)?, c, rust))
}

fn abi_header(workspace: &Path) -> Result<Vec<String>, String> {
    let (_, c, rust) = abi_declarations(workspace)?;
    let mut issues = Vec::new();
    for name in c.keys().filter(|name| !rust.contains_key(*name)) {
        issues.push(format!("abi-header: {name} is declared in include/pxui.h but has no Rust no_mangle export"));
    }
    for name in rust.keys().filter(|name| !c.contains_key(*name)) {
        issues.push(format!("abi-header: {name} is exported from pxui_abi but absent from include/pxui.h"));
    }
    for (name, declaration) in &c {
        if let Some(signature) = rust.get(name) {
            let c_sig = c_signature(declaration);
            if &c_sig != signature {
                issues.push(format!("abi-header: signature mismatch for {name}: header {c_sig}, Rust {signature}"));
            }
        }
    }
    Ok(issues)
}

fn abi_snapshot_lines(workspace: &Path) -> Result<(u32, BTreeMap<String, String>), String> {
    let (minor, exports, _) = abi_declarations(workspace)?;
    let layout_path = workspace.join("abi/layout.v1.txt");
    let layout = fs::read_to_string(&layout_path).map_err(|error| format!("{}: {error}", layout_path.display()))?;
    let mut lines = BTreeMap::new();
    for (name, declaration) in exports {
        lines.insert(format!("function {name}"), format!("function {name} {declaration}"));
    }
    for row in layout.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        let name = row.split_whitespace().next().unwrap();
        lines.insert(format!("layout {name}"), format!("layout {row}"));
    }
    Ok((minor, lines))
}

fn compare_abi_snapshot(old: &str, minor: u32, current: &BTreeMap<String, String>) -> Vec<String> {
    let mut issues = Vec::new();
    let mut old_minor = None;
    let mut old_lines = BTreeMap::new();
    for line in old.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        if let Some(value) = line.strip_prefix("minor ") {
            old_minor = value.parse::<u32>().ok();
        } else {
            let mut words = line.split_whitespace();
            if let (Some(kind), Some(name)) = (words.next(), words.next()) {
                old_lines.insert(format!("{kind} {name}"), line.to_owned());
            }
        }
    }
    let Some(old_minor) = old_minor else { return vec!["abi-snapshot: snapshot has no valid `minor N` line".to_owned()] };
    if minor < old_minor {
        issues.push(format!("abi-snapshot: ABI minor decreased from {old_minor} to {minor}"));
    }
    for (key, old_line) in &old_lines {
        match current.get(key) {
            None => issues.push(format!("abi-snapshot: removed {old_line}")),
            Some(new_line) if new_line != old_line => issues.push(format!("abi-snapshot: changed `{old_line}` to `{new_line}`")),
            _ => (),
        }
    }
    let additions: Vec<_> = current.iter().filter(|(key, _)| !old_lines.contains_key(*key)).map(|(_, line)| line).collect();
    if !additions.is_empty() && minor <= old_minor {
        issues.push(format!("abi-snapshot: {} new ABI line(s) require ABI minor > {old_minor}", additions.len()));
    }
    issues
}

fn abi_snapshot(workspace: &Path, write: bool) -> Result<Vec<String>, String> {
    let (minor, lines) = abi_snapshot_lines(workspace)?;
    let path = workspace.join("abi/pxui.v1.txt");
    let old = match fs::read_to_string(&path) {
        Ok(old) => old,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let issues = if old.is_empty() { Vec::new() } else { compare_abi_snapshot(&old, minor, &lines) };
    if !issues.is_empty() { return Ok(issues); }
    if write {
        let mut snapshot = format!("# Append-only PXUI ABI snapshot.\nminor {minor}\n");
        for line in lines.values() { snapshot.push_str(line); snapshot.push('\n'); }
        fs::write(&path, snapshot).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Vec::new())
    } else if old.is_empty() {
        Ok(vec!["abi-snapshot: abi/pxui.v1.txt is missing; create the initial baseline with `cargo run -p gate -- abi-snapshot --write`".to_owned()])
    } else {
        Ok(Vec::new())
    }
}


fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate Rust workspace root".to_owned())
}

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "all".into());
    let option = args.next();
    let write_snapshot = option.as_deref() == Some("--write");
    if args.next().is_some() || (option.is_some() && !write_snapshot) || (write_snapshot && command != "abi-snapshot") || !matches!(command.as_str(), "shape" | "deps" | "tiers" | "docs" | "pxui" | "abi-exports" | "abi-header" | "abi-snapshot" | "install-smoke" | "all") {
        eprintln!("usage: cargo run -p gate -- <shape|deps|tiers|docs|pxui|abi-exports|abi-header|abi-snapshot [--write]|install-smoke|all>");
        process::exit(2);
    }
    let workspace = match workspace_root() {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!("gate: {error}");
            process::exit(2);
        }
    };
    let result = match command.as_str() {
        "shape" => shape(&workspace),
        "deps" => deps(&workspace),
        "tiers" => tiers(&workspace),
        "docs" => docs(&workspace),
        "pxui" => pxui_remote(&workspace),
        "abi-exports" => abi_exports(&workspace),
        "abi-header" => abi_header(&workspace),
        "abi-snapshot" => abi_snapshot(&workspace, write_snapshot),
        "install-smoke" => install_smoke(&workspace),
        _ => shape(&workspace).and_then(|mut errors| {
            errors.extend(deps(&workspace)?);
            errors.extend(tiers(&workspace)?);
            errors.extend(pxui_remote(&workspace)?);
            if errors.is_empty() { errors.extend(abi_exports(&workspace)?); }
            if errors.is_empty() { errors.extend(abi_header(&workspace)?); }
            if errors.is_empty() { errors.extend(abi_snapshot(&workspace, false)?); }
            if errors.is_empty() { errors.extend(install_smoke(&workspace)?); }
            if errors.is_empty() { errors.extend(docs(&workspace)?); }
            Ok(errors)
        }),
    };
    match result {
        Ok(issues) if issues.is_empty() => println!("{} gate passed", command),
        Ok(issues) => {
            for issue in issues {
                eprintln!("{issue}");
            }
            process::exit(1);
        }
        Err(error) => {
            eprintln!("gate: {error}");
            process::exit(2);
        }
    }
}

#[cfg(test)]
mod tier_tests {
    use super::{feature_reaches_package, policy::tier_allows, DepSpec};

    #[test]
    fn tier_edges_follow_the_public_private_open_table() {
        assert!(tier_allows("private", "pxui_core", "private", "pxui_text", false, false));
        assert!(tier_allows("private", "pxui_core", "open", "input", false, false));
        assert!(tier_allows("private", "pxui_core", "public", "pxui_types", false, false));
        assert!(!tier_allows("open", "input", "private", "pxui_core", false, false));
        assert!(tier_allows("public", "pxui", "private", "pxui_core", false, false));
        assert!(tier_allows("public", "pxui", "private", "pxui_render", false, false));
        assert!(!tier_allows("public", "pxui", "private", "pxui_text", false, false));
        assert!(tier_allows("public", "pxui", "private", "pxui_abi", true, true));
        assert!(!tier_allows("public", "pxui", "private", "pxui_abi", true, false));
    }

    #[test]
    fn static_feature_resolves_dependency_alias_to_package() {
        let dep = DepSpec {
            alias: "pxui_abi_link".to_owned(),
            package: Some("pxui_abi".to_owned()),
            path: Some("../../private/pxui_abi".to_owned()),
            workspace: false,
            optional: true,
            kind: "dependencies".to_owned(),
            target: None,
            line: 1,
        };
        assert!(feature_reaches_package("[\"dep:pxui_abi_link\"]", "pxui_abi", &[dep.clone()]));
        assert!(!feature_reaches_package("[\"dep:pxui_abi_link\"]", "pxui_core", &[dep.clone()]));
        assert!(!feature_reaches_package("[\"dep:pxui_abi\"]", "pxui_abi", &[dep]));
    }
}

#[cfg(test)]
mod crate_name_tests {
    use super::graph_crate_name;

    #[test]
    fn only_the_abi_package_gets_a_distinct_graph_identity_for_the_pxui_lib_name() {
        assert_eq!(graph_crate_name("pxui", "pxui"), "pxui");
        assert_eq!(graph_crate_name("pxui_abi", "pxui"), "pxui_abi");
        assert_eq!(graph_crate_name("other", "pxui"), "pxui");
    }
}

#[cfg(test)]
mod abi_tests {
    use super::{c_signature, compare_abi_snapshot, lex, rust_exports, rust_signature};
    use std::collections::BTreeMap;

    #[test]
    fn snapshot_is_append_only_and_additions_bump_minor() {
        let old = "minor 2\nfunction pxui_a int32_t pxui_a ( void ) ;\nlayout Rect 16 4 0 4 8 12\n";
        let current = BTreeMap::from([
            ("function pxui_a".to_owned(), "function pxui_a int32_t pxui_a ( void ) ;".to_owned()),
            ("function pxui_b".to_owned(), "function pxui_b void pxui_b ( void ) ;".to_owned()),
            ("layout Rect".to_owned(), "layout Rect 16 4 0 4 8 12".to_owned()),
        ]);
        assert!(!compare_abi_snapshot(old, 2, &current).is_empty());
        assert!(compare_abi_snapshot(old, 3, &current).is_empty());

        let changed = BTreeMap::from([
            ("function pxui_a".to_owned(), "function pxui_a int32_t pxui_a ( int32_t ) ;".to_owned()),
            ("layout Rect".to_owned(), "layout Rect 16 4 0 4 8 12".to_owned()),
        ]);
        assert!(!compare_abi_snapshot(old, 4, &changed).is_empty());
    }

    #[test]
    fn c_and_rust_export_signatures_normalize_abi_types() {
        let c = c_signature("PxuiBox *pxui_get(PxuiGpu *gpu, uint32_t flags);");
        let rust_tokens = lex("pub unsafe extern \"C\" fn pxui_get(gpu: *mut Gpu, flags: u32) -> *mut Box { }");
        let at = rust_tokens.iter().position(|token| token.text == "fn").unwrap();
        let (_, rust) = rust_signature(&rust_tokens, at).unwrap();
        assert_eq!(c, rust);

        let different = c_signature("PxuiBox *pxui_get(PxuiGpu *gpu, uint64_t flags);");
        assert_ne!(different, rust);
        assert_ne!(c_signature("const PxuiBox *pxui_get(PxuiGpu *gpu, uint32_t flags);"), c);
    }

    #[test]
    fn c_parameter_arrays_decay_to_pointers() {
        let c = c_signature("int32_t pxui_style(const PxuiLookDesc states[6], const float at[2]);");
        let rust_tokens = lex("pub unsafe extern \"C\" fn pxui_style(states: *const PxuiLookDesc, at: *const f32) -> i32 { }");
        let at = rust_tokens.iter().position(|token| token.text == "fn").unwrap();
        let (_, rust) = rust_signature(&rust_tokens, at).unwrap();
        assert_eq!(c, rust);
    }

    #[test]
    fn module_exports_are_found_with_unsafe_no_mangle_attributes() {
        let exports = rust_exports("#[unsafe(no_mangle)] pub unsafe extern \"C\" fn pxui_text_create(value: *const TextStyle) -> *mut Text { }\n");
        assert_eq!(exports.get("pxui_text_create").map(String::as_str), Some("(*const TextStyle)->*mut Text"));
    }
}

#[cfg(test)]
mod export_tests {
    use super::dynamic_symbols;

    #[test]
    fn parses_macos_and_linux_dynamic_symbols() {
        assert_eq!(dynamic_symbols("macos", "0000000000001000 T _pxui_abi_version\n"), ["pxui_abi_version"]);
        assert_eq!(dynamic_symbols("linux", "0000000000001000 T pxui_abi_version\n"), ["pxui_abi_version"]);
    }

    #[test]
    fn parses_windows_gnu_export_table() {
        let output = "[Ordinal/Name Pointer] Table -- Ordinal Base 1\n  Ordinal Hint Name\n [ 0] +base[ 1] 00001330 pxui_abi_version\n\nThe Function Table\n";
        assert_eq!(dynamic_symbols("windows", output), ["pxui_abi_version"]);
    }

    #[test]
    fn parses_windows_llvm_export_table() {
        let output = "Export Table:\n DLL name: pxui_abi.dll\n Ordinal base: 1\n Ordinal      RVA  Name\n 1   0x13a0  pxui_abi_version\n\nImport Table:\n";
        assert_eq!(dynamic_symbols("windows", output), ["pxui_abi_version"]);
    }
}
