//! A preparer clones the font database and its generation before moving to a worker. The worker
//! owns shaping and glyph-list capacity; installation swaps owned storage into the UI cache and
//! leaves replaced storage in the returned record. No font discovery, shaping or allocation is
//! performed by installation. Glyph rasterization/atlas growth remain separate cold work.
use super::*;

const MAX_TEXT_BYTES: usize = 1 << 20;
/// Debug builds remember this many installed layouts' styles (a ring; no allocation after `preparer`).
#[cfg(debug_assertions)]
const MAX_INSTALLED: usize = 256;

pub struct Prepared {
    font_generation: Arc<()>,
    key: u64,
    entry: Entry,
    spare: Vec<CachedGlyph>,
    ready: bool,
}

impl Prepared {
    pub fn metrics(&self) -> Metrics { self.entry.m }
    pub fn is_ready(&self) -> bool { self.ready }
}

impl Text {
    /// Between frames: reserve the bounded receiving cache and clone immutable font sources.
    /// Later font additions/discovery invalidate this preparer's deliveries.
    pub fn preparer(&mut self) -> Text {
        self.entries.reserve(MAX_LAYOUTS - self.entries.len());
        self.keys.reserve(MAX_LAYOUTS - self.keys.len());
        #[cfg(debug_assertions)]
        self.installed.reserve_exact(MAX_INSTALLED - self.installed.len());
        let mut worker = Text::new_with_locale(&[], self.g.fs.locale(), self.atlas_size(AtlasKind::Mask), self.atlas_size(AtlasKind::Color));
        worker.g.fs = FontSystem::new_with_locale_and_db(self.g.fs.locale().into(), self.g.fs.db().clone());
        worker.names.clone_from(&self.names);
        worker.font_metrics.clone_from(&self.font_metrics);
        worker.system = self.system.clone();
        worker.font_generation = self.font_generation.clone();
        worker
    }

    /// Worker only. A result owns its source text, original shaping and reusable glyph storage.
    pub fn prepare(&mut self, text: &str, style: &Style, width: f32, spans: &[TextSpan]) -> Option<Prepared> {
        if text.len() > MAX_TEXT_BYTES || !width.is_finite() || !valid_spans(text, spans)
            || !style.size.is_finite() || style.size <= 0.0 || !style.line_height.is_finite()
            || !style.tracking.is_finite() || style.features & !Style::FEATURE_TABULAR_NUMBERS != 0 {
            return None;
        }
        let key = content_key(style, width, text, spans);
        self.layout_spans(key, text, style, width, spans)?;
        let mut entry = self.entries.remove(&key)?;
        self.keys.swap_remove(self.keys.iter().position(|&item| item == key)?);
        self.cache_bytes -= entry.glyphs.capacity() * size_of::<CachedGlyph>();
        entry.cached = false;
        entry.glyphs.clear();
        entry.shelves = [0; 8];
        let glyphs = entry.buf.layout_runs().map(|run| run.glyphs.len()).sum::<usize>();
        let parts = 2 * spans.iter().filter(|span| span.flags & TextSpan::COLOR != 0).count() + 1;
        let capacity = glyphs.checked_mul(parts)?;
        if capacity > CACHE_BYTES / size_of::<CachedGlyph>() { return None; }
        entry.glyphs.try_reserve_exact(capacity).ok()?;
        Some(Prepared { font_generation: self.font_generation.clone(), key, entry, spare: Vec::new(), ready: true })
    }

    /// Between frames. On success the record owns retired storage and can only be destroyed.
    /// Failed validation leaves both the cache and record unchanged.
    pub fn install(&mut self, record: &mut Prepared) -> bool {
        self.install_record(record, true)
    }

    /// `install`; `named`: debug builds remember its style to report a draw that asks for another (an app's prepared text, not
    /// the engine's prepare queue, whose text lays out at every width a node asks for).
    pub(crate) fn install_record(&mut self, record: &mut Prepared, named: bool) -> bool {
        #[cfg(not(debug_assertions))]
        let _ = named;
        if !record.ready || !Arc::ptr_eq(&self.font_generation, &record.font_generation)
            || self.entries.capacity() < MAX_LAYOUTS || self.keys.capacity() < MAX_LAYOUTS {
            return false;
        }
        // a replaced layout a near list row or a live view still owns keeps that owner (and a view's,
        // the frame it was painted); another (a far row of shared text, an evicted entry's reused
        // storage) is unowned, so age keeps it for its new row
        let owner = self.entries.get(&record.key)
            .filter(|e| self.list_distance(e).map_or(self.view_owned(e), |d| d <= LIST_KEEP_ROWS))
            .map_or((0, 0, 0, self.frame), |e| (e.list, e.row, e.view, if self.view_owned(e) { e.frame } else { self.frame }));
        if !self.entries.contains_key(&record.key) {
            let empty = if self.keys.len() == MAX_LAYOUTS {
                let at = self.victim();
                let old = self.keys.swap_remove(at);
                self.entries.remove(&old).expect("layout keys match their entries")
            } else {
                Entry::empty(&record.entry.style, self.frame)
            };
            self.entries.insert(record.key, empty);
            self.keys.push(record.key);
        }
        let slot = self.entries.get_mut(&record.key).expect("installed layout slot");
        let retained = self.cache_bytes - slot.glyphs.capacity() * size_of::<CachedGlyph>();
        if !self.cache_on || retained + record.entry.glyphs.capacity() * size_of::<CachedGlyph>() > CACHE_BYTES {
            std::mem::swap(&mut record.spare, &mut record.entry.glyphs);
            record.entry.cacheable = false;
        }
        std::mem::swap(slot, &mut record.entry);
        (slot.list, slot.row, slot.view, slot.frame) = owner;
        self.cache_bytes = retained + slot.glyphs.capacity() * size_of::<CachedGlyph>();
        #[cfg(debug_assertions)]
        if named && self.installed.capacity() >= MAX_INSTALLED {
            let item = (text_hash(&slot.text), record.key, slot.style, f32::from_bits(slot.width));
            match self.installed.iter().position(|old| old.1 == record.key) {
                Some(at) => self.installed[at] = item,
                None if self.installed.len() < MAX_INSTALLED => self.installed.push(item),
                None => { self.installed.remove(0); self.installed.push(item); }
            }
        }
        record.ready = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warm_glyphs(text: &mut Text, style: &Style) {
        let key = content_key(style, 200.0, "Track 0123456789", &[]);
        text.layout(key, "Track 0123456789", style, 200.0);
        for x in [0.0, 0.125, 0.25, 0.375] {
            text.glyphs(key, (x, 0.0), 2.0, &mut |_| {});
        }
    }

    #[cfg(any(target_os="macos",target_os="ios"))]
    #[test]
    fn prepared_text_reports_worker_cpu_separately_from_receiving_thread_savings() {
        use mem_sys::profile::thread_cpu_time_ns as cpu;
        let style=Style {size:13.0,features:Style::FEATURE_TABULAR_NUMBERS,..Default::default()};
        let sources=(0..64).map(|index|format!("Track {index:05}")).collect::<Vec<_>>();
        let mut ordinary=Text::new(&[PRAGMASEVKA]);
        warm_glyphs(&mut ordinary,&style);
        let mut ui=Text::new(&[PRAGMASEVKA]);
        warm_glyphs(&mut ui,&style);
        let mut worker=ui.preparer();
        mem_sys::local_reset();
        let start=cpu().unwrap();
        let mut ordinary_glyphs=0;
        for source in &sources {
            let key=content_key(&style,200.0,source,&[]);
            ordinary.layout(key,source,&style,200.0);
            ordinary.glyphs(key,(0.0,0.0),2.0,&mut |_|ordinary_glyphs+=1);
        }
        let ordinary_ns=cpu().unwrap()-start;
        let ordinary_allocations=mem_sys::local_alloc_count();
        assert!(ordinary_allocations>0);
        let (sources,mut records,worker_ns)=std::thread::spawn(move || {
            let start=cpu().unwrap();
            let records=sources.iter().map(|source|worker.prepare(source,&style,200.0,&[]).unwrap()).collect::<Vec<_>>();
            let elapsed=cpu().unwrap()-start;
            (sources,records,elapsed)
        }).join().unwrap();
        let layouts=ui.layout_count();
        mem_sys::local_reset();
        let start=cpu().unwrap();
        let mut prepared_glyphs=0;
        for (source,record) in sources.iter().zip(&mut records) {
            let expected=record.metrics();
            assert!(ui.install(record));
            let key=content_key(&style,200.0,source,&[]);
            assert_eq!(ui.layout(key,source,&style,200.0),expected);
            ui.glyphs(key,(0.0,0.0),2.0,&mut |_|prepared_glyphs+=1);
        }
        let prepared_ns=cpu().unwrap()-start;
        assert_eq!(mem_sys::local_alloc_count(),0);
        assert_eq!(ui.layout_count(),layouts);
        assert_eq!(prepared_glyphs,ordinary_glyphs);
        eprintln!("prepared text 64 new labels, warm glyphs: ordinary main {ordinary_ns} ns/{ordinary_allocations} allocations; prepared main {prepared_ns} ns/0 allocations; worker {worker_ns} ns (separate thread, not a total CPU reduction)");
        std::thread::spawn(move ||drop(records)).join().unwrap();
    }

    #[test]
    fn worker_layouts_install_without_shaping_or_allocating_and_return_old_storage() {
        let style = Style { size: 13.0, features: Style::FEATURE_TABULAR_NUMBERS, ..Style::default() };
        let mut ui = Text::new(&[PRAGMASEVKA]);
        warm_glyphs(&mut ui, &style);
        let mut worker = ui.preparer();
        let records = std::thread::spawn(move || {
            (0..64).map(|index| worker.prepare(&format!("Track {index:05}"), &style, 200.0, &[]).unwrap()).collect::<Vec<_>>()
        }).join().unwrap();
        let layouts = ui.layout_count();
        let mut records = records;
        mem_sys::local_reset();
        for record in &mut records {
            let key = record.key;
            let expected = record.metrics();
            assert!(ui.install(record));
            assert!(!record.is_ready());
            assert!(!ui.install(record), "a retirement record cannot be installed again");
            assert_eq!(ui.entries[&key].m, expected);
            let mut glyphs = 0;
            ui.glyphs(key, (0.0, 0.0), 2.0, &mut |_| glyphs += 1);
            assert!(glyphs > 5);
        }
        assert_eq!(ui.layout_count(), layouts, "installation must not shape on the receiving thread");
        assert_eq!(mem_sys::local_alloc_count(), 0);
        assert_eq!(ui.keys.len(), ui.entries.len());
        let mut worker = ui.preparer();
        let mut replacement = std::thread::spawn(move || worker.prepare("Track 00000", &style, 200.0, &[]).unwrap()).join().unwrap();
        mem_sys::local_reset();
        assert!(ui.install(&mut replacement));
        assert_eq!(replacement.entry.text, "Track 00000", "replaced storage travels back in the record");
        assert!(replacement.entry.glyphs.capacity() > 0);
        assert_eq!(mem_sys::local_alloc_count(), 0);
        std::thread::spawn(move || drop((records, replacement))).join().unwrap();
    }

    #[test]
    fn reinstalling_a_layout_keeps_its_list_and_view_ownership() {
        let style = Style { size: 13.0, ..Style::default() };
        let mut ui = Text::new(&[PRAGMASEVKA]);
        let mut worker = ui.preparer();
        let (mut first, mut again, mut later) = std::thread::spawn(move || {
            let mut album = || worker.prepare("Album 70", &style, 200.0, &[]).unwrap();
            (album(), album(), album())
        }).join().unwrap();
        let key = first.key;
        assert!(ui.install(&mut first));
        ui.list_window(7, 0, 40);
        ui.list_row(7, 30);
        let outer = ui.paint_view(2);
        ui.layout(key, "Album 70", &style, 200.0);
        ui.glyphs(key, (0.0, 0.0), 1.0, &mut |_| {});
        ui.paint_view_end(outer);
        ui.list_end();
        assert!(ui.install(&mut again));
        let entry = &ui.entries[&key];
        assert!((entry.list, entry.row, entry.view) == (7, 30, 2) && ui.view_owned(entry));
        for _ in 0..2 * KEEP_FRAMES {
            ui.end_frame();
        }
        assert!(ui.entries.contains_key(&key), "a reinstalled row in its list's window survives idle");
        // the same text prepared again for a row far below: row 30 no longer holds it, age does
        ui.list_window(7, 5000, 5040);
        assert!(ui.install(&mut later));
        assert_eq!(ui.entries[&key].list, 0);
        // a full cache of far list rows: the new layout reuses an evicted one's storage, not its ownership
        let mut worker = ui.preparer();
        let mut fresh = std::thread::spawn(move || worker.prepare("Album 71", &style, 200.0, &[]).unwrap()).join().unwrap();
        for row in 0..MAX_LAYOUTS as u64 {
            ui.list_row(7, row as usize);
            ui.layout(10_000 + row, "", &style, 0.0);
            ui.list_end();
        }
        let key = fresh.key;
        assert!(ui.install(&mut fresh));
        let entry = &ui.entries[&key];
        assert_eq!((entry.list, entry.view), (0, 0));
        std::thread::spawn(move || drop((first, again, later, fresh))).join().unwrap();
    }

    #[test]
    fn stale_or_foreign_fonts_and_invalid_preparation_leave_the_cache_unchanged() {
        let mut ui = Text::new(&[PRAGMASEVKA]);
        let mut worker = ui.preparer();
        let mut record = worker.prepare("hello", &Style::default(), 200.0, &[]).unwrap();
        let mut other = Text::new(&[PRAGMASEVKA]);
        let _ = other.preparer();
        assert!(!other.install(&mut record));
        assert!(record.is_ready());
        assert!(other.entries.is_empty());
        ui.add_font(PRAGMASEVKA);
        assert!(!ui.install(&mut record));
        assert!(record.is_ready());
        assert!(ui.entries.is_empty());
        assert!(worker.prepare("hello", &Style::default(), f32::NAN, &[]).is_none());
        assert!(worker.prepare("hello", &Style { size: 0.0, ..Style::default() }, 200.0, &[]).is_none());
        assert!(worker.prepare("é", &Style::default(), 200.0, &[TextSpan { start: 1, end: 2, ..Default::default() }]).is_none());
    }

    #[cfg(target_os="macos")]
    #[test]
    fn native_font_discovery_on_either_side_invalidates_prepared_face_identities() {
        let mut ui=Text::new(&[PRAGMASEVKA]);
        assert!(ui.enable_system_fonts());
        let mut worker=ui.preparer();
        let mut record=worker.prepare("plain",&Style::default(),200.0,&[]).unwrap();
        ui.layout(1,"漢字",&Style::default(),200.0);
        assert!(!ui.system.as_ref().unwrap().paths.is_empty());
        assert!(!ui.install(&mut record));
        assert!(record.is_ready());
        let mut worker=ui.preparer();
        let mut record=worker.prepare("😀",&Style::default(),200.0,&[]).unwrap();
        assert!(worker.system.as_ref().unwrap().paths.len()>ui.system.as_ref().unwrap().paths.len());
        assert!(!ui.install(&mut record));
        assert!(record.is_ready());
        // Discover on the UI, then fork again: matching fallback face IDs are now transferable.
        ui.layout(2,"😀",&Style::default(),200.0);
        let mut worker=ui.preparer();
        let mut record=worker.prepare("😀漢字",&Style::default(),200.0,&[]).unwrap();
        assert!(ui.install(&mut record));
    }

    #[test]
    fn prepared_cache_capacity_and_eviction_remain_bounded_without_warm_allocations() {
        let mut ui = Text::new(&[PRAGMASEVKA]);
        let mut worker = ui.preparer();
        let mut records = std::thread::spawn(move || {
            (0..MAX_LAYOUTS * 3 + 8).map(|index| worker.prepare(&index.to_string(), &Style::default(), 0.0, &[]).unwrap()).collect::<Vec<_>>()
        }).join().unwrap();
        mem_sys::local_reset();
        for record in &mut records { assert!(ui.install(record)); }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        assert_eq!(ui.entries.len(), MAX_LAYOUTS);
        assert_eq!(ui.keys.len(), MAX_LAYOUTS);
        assert!(ui.cache_bytes <= CACHE_BYTES);
        std::thread::spawn(move || drop(records)).join().unwrap();
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_draw_with_another_style_than_the_prepared_one_reports_both_styles_once() {
        let text = "style-mismatch-probe";
        let plain = Style::default();
        let ellipsized = Style { ellipsis: Ellipsis::End, max_lines: 1, ..Style::default() };
        let mine = || mem_sys::take_reports().into_iter().filter(|report| report.contains(text)).collect::<Vec<_>>();
        let mut ui = Text::new(&[PRAGMASEVKA]);
        ui.reporter = Some(mem_sys::report);
        let mut worker = ui.preparer();
        let mut record = worker.prepare(text, &plain, 80.0, &[]).unwrap();
        assert!(ui.install(&mut record));
        mine();
        // the matching request adopts the installed layout
        ui.layout(content_key(&plain, 80.0, text, &[]), text, &plain, 80.0);
        assert!(mine().is_empty());
        // the wrong-ellipsis request shapes afresh and is named once; asking again hits the cache
        let key = content_key(&ellipsized, 80.0, text, &[]);
        ui.layout(key, text, &ellipsized, 80.0);
        ui.layout(key, text, &ellipsized, 80.0);
        let reports = mine();
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert!(reports[0].contains("ellipsis: None") && reports[0].contains("ellipsis: End"), "{}", reports[0]);
    }

    #[test]
    fn uncached_prepared_glyph_storage_returns_to_the_worker() {
        let mut ui = Text::new(&[PRAGMASEVKA]);
        let style = Style::default();
        warm_glyphs(&mut ui, &style);
        let mut worker = ui.preparer();
        let mut record = worker.prepare("Track 012", &style, 200.0, &[]).unwrap();
        let key = record.key;
        ui.set_glyph_cache(false);
        let retained = ui.glyph_list_bytes();
        mem_sys::local_reset();
        assert!(ui.install(&mut record));
        ui.glyphs(key, (0.0, 0.0), 2.0, &mut |_| {});
        assert_eq!(mem_sys::local_alloc_count(), 0);
        assert!(!ui.entries[&key].cacheable);
        assert!(record.spare.capacity() > 0);
        assert_eq!(ui.glyph_list_bytes(), retained);
    }

    #[test]
    fn prepared_glyph_capacity_cannot_exceed_the_receiving_cache_budget() {
        let mut ui = Text::new(&[PRAGMASEVKA]);
        let mut worker = ui.preparer();
        let mut records = std::thread::spawn(move || {
            (0..64).map(|index| worker.prepare(&format!("{index:02} {}", "a".repeat(1024)), &Style::default(), 0.0, &[]).unwrap()).collect::<Vec<_>>()
        }).join().unwrap();
        mem_sys::local_reset();
        for record in &mut records { assert!(ui.install(record)); }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        assert!(ui.glyph_list_bytes() <= CACHE_BYTES);
        assert!(ui.entries.values().any(|entry| !entry.cacheable));
        assert!(records.iter().any(|record| record.spare.capacity() > 0));
        std::thread::spawn(move || drop(records)).join().unwrap();
    }

    #[test]
    fn prepared_layouts_preserve_wrapping_ellipsis_spans_bidi_and_fractional_glyphs() {
        let fonts: &[&[u8]] = &[PRAGMASEVKA, include_bytes!("../../../../assets/fonts/NotoSansArabic.ttf")];
        let mut ui = Text::new(fonts);
        let mut reference = Text::new(fonts);
        reference.set_glyph_cache(false);
        let mut worker = ui.preparer();
        for source in ["لا", "abc العربية e\u{301}", "one two\r\nthree four", "long-file-name.extension", ""] {
            let spans = if let Some(first) = source.chars().next() {
                vec![TextSpan { start: 0, end: first.len_utf8() as u32, weight: 500,
                    flags: TextSpan::COLOR | TextSpan::BACKGROUND | TextSpan::UNDERLINE,
                    color: pxui_types::geom::Rgba { r: 255, g: 0, b: 0, a: 255 },
                    background: pxui_types::geom::Rgba { r: 0, g: 0, b: 255, a: 255 },
                    underline: pxui_types::geom::Rgba { r: 0, g: 255, b: 0, a: 255 },
                }]
            } else { Vec::new() };
            for mode in [Ellipsis::None, Ellipsis::End, Ellipsis::Middle, Ellipsis::Head] {
                let style = Style { size: 20.25, line_height: 26.0, wrap: Wrap::WordSpaces, max_lines: 2, ellipsis: mode, ..Style::default() };
                let worker_key=content_key(&style,64.0,source,&spans);
                worker.layout_spans(worker_key,source,&style,64.0,&spans).unwrap();
                worker.glyphs(worker_key,(7.0,3.0),2.0,&mut |_|{});
                let mut record = worker.prepare(source, &style, 64.0, &spans).unwrap();
                assert!(record.entry.glyphs.is_empty(),"worker atlas bindings must never reach the UI");
                assert_eq!(record.entry.shelves,[0;8]);
                let key = record.key;
                let expected = reference.layout_spans(key, source, &style, 64.0, &spans).unwrap();
                assert_eq!(record.metrics(), expected);
                assert!(ui.install(&mut record));
                for scale in [1.0, 2.0, 1.5] {
                    let mut actual = Vec::new();
                    let mut expected = Vec::new();
                    ui.glyphs(key, (-2.375, 5.5), scale, &mut |glyph| actual.push(glyph));
                    reference.glyphs(key, (-2.375, 5.5), scale, &mut |glyph| expected.push(glyph));
                    assert_eq!(actual, expected, "{source:?}, {mode:?}, {scale}x");
                }
                for underline in [false, true] {
                    let mut actual = Vec::new();
                    let mut expected = Vec::new();
                    ui.decorations(key, underline, &mut |decoration| actual.push(decoration));
                    reference.decorations(key, underline, &mut |decoration| expected.push(decoration));
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
