//! The layout manifest (`abi/layout.v1.txt`) and the C layout asserts at the end of `include/pxui.h`, both generated from the Rust types.
//! Each layout test reports its rows through `row`; `abi_layout_rows_match_the_files` renders the manifest rows and the header block from
//! them and fails when either file differs. `PXUI_LAYOUT_WRITE=1 cargo t -p pxui_types abi_layout_rows` rewrites both files.

use std::cell::RefCell;
use std::fmt::Write;

struct Row {
    name: String,
    size: usize,
    align: usize,
    fields: Vec<(String, usize)>,
}

thread_local! { static ROWS: RefCell<Vec<Row>> = const { RefCell::new(Vec::new()) }; }

/// Records one type's size, alignment and field offsets. A row reported twice must agree.
pub(crate) fn row(name: &str, size: usize, align: usize, fields: &[(&str, usize)]) {
    let fields: Vec<(String, usize)> = fields.iter().map(|(f, o)| ((*f).to_owned(), *o)).collect();
    ROWS.with(|rows| {
        let mut rows = rows.borrow_mut();
        match rows.iter().find(|r| r.name == name) {
            Some(r) => assert!(r.size == size && r.align == align && r.fields == fields, "{name} is reported twice with different layouts"),
            None => rows.push(Row { name: name.to_owned(), size, align, fields }),
        }
    });
}

fn manifest_line(r: &Row) -> String {
    let mut line = format!("{} {} {}", r.name, r.size, r.align);
    for (_, offset) in &r.fields {
        write!(line, " {offset}").unwrap();
    }
    line
}

/// The C type of a manifest row; `None` for the rows `pxui.h` has no type for.
fn c_name(row: &str) -> Option<String> {
    if ["Rgba", "Flags", "Role", "View", "Slice"].contains(&row) { return None }
    if row.starts_with("FrameInV1_") { return Some("PxuiFrameIn".to_owned()) }
    if row == "PxuiUiDescV1_60" { return Some("PxuiUiDesc".to_owned()) }
    Some(if row.starts_with("Pxui") { row.to_owned() } else { format!("Pxui{row}") })
}

/// The frozen `FrameIn` prefixes are as long as the offset of the field that follows them; `FrameInV1_57` is the whole struct; so is `PxuiUiDescV1_60` (`PxuiUiDesc` is its prefix).
fn c_size(row: &str, c: &str) -> String {
    let next = match row { "FrameIn" => "appearance", "FrameInV1_8" => "environment", "FrameInV1_24" => "keyboard", "FrameInV1_26" => "surface", "PxuiUiDesc" => "abi_built", _ => return format!("sizeof({c})") };
    format!("offsetof({c}, {next})")
}

/// The C name of a Rust field where `pxui.h` flattens a slice into `ptr, len` or names an array differently.
fn c_field<'a>(row: &str, field: &'a str) -> &'a str {
    match (row, field) {
        ("PxuiFooterDesc" | "PxuiTreeViewDesc", "hints" | "nodes") => "ptr",
        ("PxuiThemeSnapshot", "panel") => "parts",
        ("PxuiThemeSnapshot", "body") => "bodies",
        _ => field,
    }
}

fn c_assert(r: &Row) -> Option<String> {
    let c = c_name(&r.name)?;
    let mut text = format!("_Static_assert({} == {}", c_size(&r.name, &c), r.size);
    for (field, offset) in &r.fields {
        write!(text, " && offsetof({c}, {}) == {offset}", c_field(&r.name, field)).unwrap();
    }
    write!(text, ", \"{} layout\");", r.name).unwrap();
    Some(text)
}

const BEGIN: &str = "/* abi-layout begin: generated from the Rust types by `PXUI_LAYOUT_WRITE=1 cargo t -p pxui_types abi_layout_rows`; do not edit */";
const END: &str = "/* abi-layout end */";

/// The manifest with each row replaced by its generated text (comments and order kept), a repeated row dropped and rows the file lacks
/// appended, so a file that repeats a row disagrees with it.
fn render_manifest(old: &str, rows: &[Row]) -> Result<String, String> {
    let mut seen = Vec::new();
    let mut out = String::new();
    for line in old.lines() {
        let name = line.split_whitespace().next().filter(|n| !n.starts_with('#'));
        match name {
            Some(name) if seen.iter().any(|seen| seen == name) => continue,
            Some(name) => {
                // a row whose record was deleted (D28: 1.x records change in place) leaves the rendering, so the check reports it
                let Some(r) = rows.iter().find(|r| r.name == name) else { continue };
                seen.push(name.to_owned());
                out.push_str(&manifest_line(r));
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    for r in rows.iter().filter(|r| !seen.contains(&r.name)) {
        out.push_str(&manifest_line(r));
        out.push('\n');
    }
    Ok(out)
}

/// The extension manifest (`abi/pxui.ext.txt`) with each `layout` row regenerated; its function rows and comments are kept.
/// The rows it names are the extension set: `layout.v1.txt` carries none of them.
fn render_extension(old: &str, rows: &[Row]) -> Result<String, String> {
    let (mut out, mut seen) = (String::new(), Vec::new());
    for line in old.lines() {
        match line.strip_prefix("layout ").and_then(|row| row.split_whitespace().next()) {
            Some(name) if seen.contains(&name) => continue,
            Some(name) => {
                seen.push(name);
                let r = rows.iter().find(|r| r.name == name).ok_or_else(|| format!("extension row `{name}` has no Rust type reporting it"))?;
                out.push_str("layout ");
                out.push_str(&manifest_line(r));
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    Ok(out)
}

/// The names of the extension set's layout rows.
fn extension_names(ext: &str) -> Vec<String> {
    ext.lines().filter_map(|l| l.strip_prefix("layout ")).filter_map(|r| r.split_whitespace().next()).map(str::to_owned).collect()
}

/// `pxui.h` with the block between the markers regenerated, in manifest order; the block is added before the final `#endif` when absent.
fn render_header(old: &str, manifest: &str, rows: &[Row]) -> String {
    let mut block = format!("{BEGIN}\n");
    let mut done = Vec::new();
    for name in manifest.lines().filter_map(|l| l.split_whitespace().next()).filter(|n| !n.starts_with('#')) {
        if let Some(text) = rows.iter().find(|r| r.name == name).and_then(c_assert) {
            if !done.contains(&text) { block.push_str(&text); block.push('\n'); done.push(text); }
        }
    }
    block.push_str(END);
    block.push('\n');
    match (old.find(BEGIN), old.find(END)) {
        (Some(start), Some(end)) => format!("{}{}{}", &old[..start], block, &old[end + END.len() + 1..]),
        _ => {
            let at = old.rfind("#endif").expect("pxui.h ends with #endif");
            format!("{}\n{}\n{}", old[..at].trim_end(), block, &old[at..])
        }
    }
}

/// A manifest that repeats a row disagrees with its rendering (the U2a.3 merge left `PxuiSectionIndexConfig` twice and every gate passed).
#[test]
fn a_repeated_layout_row_fails_the_check() {
    let rows = [Row { name: "PxuiA".to_owned(), size: 4, align: 4, fields: vec![("a".to_owned(), 0)] }];
    let once = "# rows\nPxuiA 4 4 0\n";
    assert_eq!(render_manifest(once, &rows).unwrap(), once);
    assert_eq!(render_manifest("# rows\nPxuiA 4 4 0\nPxuiA 4 4 0\n", &rows).unwrap(), once);
    assert_eq!(render_extension("layout PxuiA 4 4 0\nlayout PxuiA 4 4 0\n", &rows).unwrap(), "layout PxuiA 4 4 0\n");
    assert_eq!(render_manifest("# rows\nPxuiGone 8 8 0\nPxuiA 4 4 0\n", &rows).unwrap(), once, "a deleted record's row leaves the manifest");
}

#[test]
fn abi_layout_rows_match_the_files() {
    ROWS.with(|rows| rows.borrow_mut().clear());
    crate::ffi::layout::ffi_rows();
    crate::abi_layout::abi_descriptors_rows();
    crate::abi_layout::value_types_rows();
    crate::abi_layout::frozen_rows();
    let rows = ROWS.with(|rows| std::mem::take(&mut *rows.borrow_mut()));
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
    let (manifest_path, header_path, ext_path) = (format!("{root}/abi/layout.v1.txt"), format!("{root}/include/pxui.h"), format!("{root}/abi/pxui.ext.txt"));
    let extension = render_extension(&std::fs::read_to_string(&ext_path).unwrap(), &rows).unwrap();
    let extended = extension_names(&extension);
    let frozen: Vec<Row> = rows.iter().filter(|r| !extended.contains(&r.name)).map(|r| Row { name: r.name.clone(), size: r.size, align: r.align, fields: r.fields.clone() }).collect();
    let manifest = render_manifest(&std::fs::read_to_string(&manifest_path).unwrap(), &frozen).unwrap();
    // the C asserts follow the frozen rows, then the extension's
    let order: String = manifest.lines().map(|l| format!("{l}\n")).chain(extension.lines().filter_map(|l| l.strip_prefix("layout ")).map(|l| format!("{l}\n"))).collect();
    let header = render_header(&std::fs::read_to_string(&header_path).unwrap(), &order, &rows);
    if std::env::var_os("PXUI_LAYOUT_WRITE").is_some() {
        std::fs::write(&manifest_path, &manifest).unwrap();
        std::fs::write(&header_path, &header).unwrap();
        std::fs::write(&ext_path, &extension).unwrap();
        return;
    }
    for (path, text) in [(&manifest_path, &manifest), (&header_path, &header), (&ext_path, &extension)] {
        let old = std::fs::read_to_string(path).unwrap();
        if let Some((n, (have, want))) = old.lines().zip(text.lines()).enumerate().find(|(_, (a, b))| a != b) {
            panic!("{path} line {} disagrees with the Rust types:\n  file: {have}\n  types: {want}\nrerun with PXUI_LAYOUT_WRITE=1 only if the type change is intended", n + 1);
        }
        assert_eq!(old.lines().count(), text.lines().count(), "{path} has a different number of lines than the Rust types produce");
    }
}
