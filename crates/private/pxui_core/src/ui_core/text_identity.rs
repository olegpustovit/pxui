use pxui_types::Flags;
use pxui_text::{Text,Metrics,Style,TextSpan};

// Reuse the layout's retained text for equality; no second string buffer or warm byte hashing.
// `ahead`: the node is in a list row past the viewport; text the cache has not seen goes to the prepare queue
// (`Text::defer`) and lays out empty this frame.
pub(super) fn lay_out_node(t:&mut Text,memo:&std::cell::Cell<Option<(u64,u64)>>,flags:Flags,style:&Style,width:f32,value:&str,spans:&[TextSpan],ahead:bool)->Option<(u64,Metrics)> {
    let fingerprint=memo.get().filter(|(key,_)|flags.any(Flags::FIT_TEXT_PIXEL) && t.has_text(*key,value))
        .map_or_else(||pxui_text::text_hash(value),|(_,fingerprint)|fingerprint);
    if spans.is_empty() && width>0.0 {
        // the node's last layout (a measure at another width), or the cached natural one (prewarmed), when this width cannot change it
        let natural=pxui_text::layout_key(style,0.0,fingerprint,&[]);
        let last=memo.get().map(|(key,_)|key).filter(|key|*key!=natural);
        for key in last.into_iter().chain(Some(natural)) {
            if let Some(metrics)=t.reusable_layout(key,value,style,width) {
                if flags.any(Flags::FIT_TEXT_PIXEL) {memo.set(Some((key,fingerprint)));}
                return Some((key,metrics));
            }
        }
    }
    let key=pxui_text::layout_key(style,width,fingerprint,spans);
    if ahead && t.defer(key,value,style,width,spans) { return Some((key,Metrics::default())); }
    let metrics=t.layout_spans(key,value,style,width,spans)?;
    if flags.any(Flags::FIT_TEXT_PIXEL) {memo.set(Some((key,fingerprint)));}
    Some((key,metrics))
}

