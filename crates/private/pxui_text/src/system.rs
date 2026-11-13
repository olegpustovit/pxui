use cosmic_text::FontSystem;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct SystemFonts {
    pub paths:HashSet<PathBuf>,
    failed:HashSet<String>,
}

impl SystemFonts {
    pub fn resolve(&mut self,cluster:&str,fonts:&mut FontSystem)->bool {
        if self.failed.contains(cluster) { return false; }
        let loaded=font_sys::fallback_path(cluster,fonts.locale()).is_some_and(|path| {
            // shortcut: at most 128 mapped fallback files; revisit with a corpus exceeding that set.
            if self.paths.len()>=128 || !self.paths.insert(path.clone()) { return false; }
            let before=fonts.db().faces().count();
            fonts.db_mut().load_font_file(&path).is_ok() && fonts.db().faces().count()>before
        });
        if !loaded {
            // Bound negative caching; clearing only makes a later cold miss query the OS again.
            if self.failed.len()>=512 { self.failed.clear(); }
            if cluster.len()<=256 { self.failed.insert(cluster.into()); }
        }
        loaded
    }
}

pub(crate) fn missing<'a>(buffer:&'a cosmic_text::Buffer,seen:&HashSet<String>)->Option<&'a str> {
    buffer.layout_runs().flat_map(|run|run.glyphs.iter().filter(|g|g.glyph_id==0)
        .filter_map(move |g|run.text.get(g.start..g.end)))
        .find(|cluster|!cluster.is_empty() && !cluster.chars().all(char::is_control) && !seen.contains(*cluster))
}

/// Resolve every newly encountered missing cluster, then shape again with all newly mapped faces.
pub(crate) fn shape(buffer:&mut cosmic_text::Buffer,fonts:&mut FontSystem,system:&mut SystemFonts) {
    let mut seen=HashSet::new();
    while let Some(cluster)=missing(buffer,&seen) {
        let cluster=cluster.to_owned();
        seen.insert(cluster.clone());
        if system.resolve(&cluster,fonts) {
            for line in &mut buffer.lines { line.reset_shaping(); }
            buffer.shape_until_scroll(fonts,true);
        }
    }
}

#[cfg(all(test,target_os="macos"))]
mod tests {
    use crate::{Text,Style,AtlasKind,PRAGMASEVKA};

    fn missing(text:&Text,key:u64)->usize {
        text.entries[&key].buf.layout_runs().flat_map(|run|run.glyphs.iter()).filter(|glyph|glyph.glyph_id==0).count()
    }

    #[test]
    fn system_fonts_resolve_missing_scripts_and_retain_warm_shapes() {
        let mut text=Text::new(&[PRAGMASEVKA]);
        let style=Style { size:20.0,..Default::default() };
        text.layout(1,"漢字",&style,300.0);
        assert!(missing(&text,1)>0,"explicit-font constructor must stay deterministic");
        assert!(text.enable_system_fonts());
        let cases=["漢字","العربية","⌘⌥⇧","😀","👩‍💻","Ж"];
        for (i,source) in cases.into_iter().enumerate() {
            text.layout(i as u64+1,source,&style,300.0);
            assert_eq!(missing(&text,i as u64+1),0,"native fallback must shape {source:?}: {:?}",text.system.as_ref().unwrap().paths);
            let mut count=0;
            let mut color=false;
            text.glyphs(i as u64+1,(0.0,0.0),1.0,&mut |g| { count+=1; color|=g.atlas==AtlasKind::Color; });
            assert!(count>0,"fallback must also rasterize {source:?}: {:?} {:?}",text.system.as_ref().unwrap().paths,text.entries[&(i as u64+1)].buf.layout_runs().collect::<Vec<_>>());
            if source=="😀" || source=="👩‍💻" { assert!(color,"native emoji uses the colour atlas"); }
        }
        assert!(!text.system.as_ref().unwrap().paths.is_empty());
        let files=text.system.as_ref().unwrap().paths.len();
        let walks=text.glyph_walks();
        for _ in 0..4 {
            mem_sys::local_reset();
            assert!(text.enable_system_fonts(),"enabling again preserves caches");
            for (i,source) in cases.into_iter().enumerate() {
                text.layout(i as u64+1,source,&style,300.0);
                text.glyphs(i as u64+1,(0.0,0.0),1.0,&mut |_|{});
            }
            text.end_frame();
            assert_eq!(mem_sys::local_alloc_count(),0,"native fallback layouts and glyphs stay allocation-free when warm");
        }
        assert_eq!(text.glyph_walks(),walks);
        assert_eq!(text.system.as_ref().unwrap().paths.len(),files);
    }

    #[test]
    fn unsupported_clusters_are_cached_and_negative_storage_is_bounded() {
        let mut text=Text::new(&[PRAGMASEVKA]);
        text.enable_system_fonts();
        let system=text.system.as_mut().unwrap();
        assert!(!system.resolve("\u{10ffff}",&mut text.g.fs));
        assert!(system.failed.contains("\u{10ffff}"));
        mem_sys::local_reset();
        for _ in 0..4 { assert!(!system.resolve("\u{10ffff}",&mut text.g.fs)); }
        assert_eq!(mem_sys::local_alloc_count(),0,"known unsupported clusters do not query the provider again");
        for i in 0..512 { system.failed.insert(format!("{i}")); }
        system.resolve("\u{10fffe}",&mut text.g.fs);
        assert!(system.failed.len()<=512);
    }

    #[test]
    fn system_only_shapers_bootstrap_a_real_font_before_shaping() {
        let mut text=Text::new(&[]);
        assert!(text.enable_system_fonts());
        let metrics=text.layout(1,"ABC 漢字 😀",&Style::default(),300.0);
        assert!(metrics.w>0.0 && metrics.lines==1);
        assert_eq!(missing(&text,1),0);
        let mut glyphs=0;
        text.glyphs(1,(0.0,0.0),1.0,&mut |_|glyphs+=1);
        assert!(glyphs>=6);
    }
}
