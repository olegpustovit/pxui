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
    Some(if row.starts_with("Pxui") { row.to_owned() } else { format!("Pxui{row}") })
}

/// The frozen `FrameIn` prefixes are as long as the offset of the field that follows them; `FrameInV1_26` is the whole struct.
fn c_size(row: &str, c: &str) -> String {
    let next = match row { "FrameIn" => "appearance", "FrameInV1_8" => "environment", "FrameInV1_24" => "keyboard", _ => return format!("sizeof({c})") };
    format!("offsetof({c}, {next})")
}

/// The C name of a Rust field where `pxui.h` flattens a slice into `ptr, len` or names an array differently.
fn c_field<'a>(row: &str, field: &'a str) -> &'a str {
    match (row, field) {
        ("PxuiFooterDesc" | "PxuiTreeViewDesc" | "PxuiTreeViewWithDesc", "hints" | "nodes") => "ptr",
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

/// The manifest with each row replaced by its generated text (comments and order kept) and rows the file lacks appended.
fn render_manifest(old: &str, rows: &[Row]) -> Result<String, String> {
    let mut seen = Vec::new();
    let mut out = String::new();
    for line in old.lines() {
        let name = line.split_whitespace().next().filter(|n| !n.starts_with('#'));
        match name {
            Some(name) => {
                let r = rows.iter().find(|r| r.name == name).ok_or_else(|| format!("manifest row `{name}` has no Rust type reporting it"))?;
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

#[test]
fn abi_layout_rows_match_the_files() {
    ROWS.with(|rows| rows.borrow_mut().clear());
    crate::ffi::layout::ffi_rows();
    crate::abi_layout::abi_descriptors_rows();
    crate::abi_layout::value_types_rows();
    crate::abi_layout::frozen_rows();
    let rows = ROWS.with(|rows| std::mem::take(&mut *rows.borrow_mut()));
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
    let (manifest_path, header_path) = (format!("{root}/abi/layout.v1.txt"), format!("{root}/include/pxui.h"));
    let manifest = render_manifest(&std::fs::read_to_string(&manifest_path).unwrap(), &rows).unwrap();
    let header = render_header(&std::fs::read_to_string(&header_path).unwrap(), &manifest, &rows);
    if std::env::var_os("PXUI_LAYOUT_WRITE").is_some() {
        std::fs::write(&manifest_path, &manifest).unwrap();
        std::fs::write(&header_path, &header).unwrap();
        return;
    }
    for (path, text) in [(&manifest_path, &manifest), (&header_path, &header)] {
        let old = std::fs::read_to_string(path).unwrap();
        if let Some((n, (have, want))) = old.lines().zip(text.lines()).enumerate().find(|(_, (a, b))| a != b) {
            panic!("{path} line {} disagrees with the Rust types:\n  file: {have}\n  types: {want}\nrerun with PXUI_LAYOUT_WRITE=1 only if the type change is intended", n + 1);
        }
        assert_eq!(old.lines().count(), text.lines().count(), "{path} has a different number of lines than the Rust types produce");
    }
}
