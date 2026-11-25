use pxui_types::Flags;
use pxui_text::{Text,Metrics,Style,TextSpan};

// Reuse the layout's retained text for equality; no second string buffer or warm byte hashing.
// `ahead`: the node is in a list row past the viewport; text the cache has not seen goes to the prepare queue
// (`Text::defer`) and lays out empty this frame.
//
// When the text bytes change, rekey the previous entry onto the new content key (`layout_spans_from`)
// so cosmic-text reshapes the same `Buffer` in place. Style/density/width key changes keep every
// prior layout (recycled FIT_TEXT_PIXEL identities cycle those). Never rekey onto a key that already
// holds an entry (prepare-queue install).
pub(super) fn lay_out_node(t:&mut Text,memo:&std::cell::Cell<Option<(u64,u64)>>,flags:Flags,style:&Style,width:f32,value:&str,spans:&[TextSpan],ahead:bool)->Option<(u64,Metrics)> {
    let previous=memo.get();
    let keep=flags.any(Flags::FIT_TEXT_PIXEL);
    let reuse=spans.is_empty() && width>0.0;
    if reuse && keep {
        if let Some((key,_))=previous {
            if let Some(metrics)=t.reusable_layout(key,value,style,width) { return Some((key,metrics)); }
        }
    }
    let fingerprint=previous.filter(|(key,_)|keep && t.has_text(*key,value))
        .map_or_else(||pxui_text::text_hash(value),|(_,fingerprint)|fingerprint);
    if reuse {
        // the node's last layout (a measure at another width), or the cached natural one (prewarmed), when this width cannot change it
        let natural=pxui_text::layout_key(style,0.0,fingerprint,&[]);
        let last=previous.map(|(key,_)|key).filter(|key|!keep && *key!=natural);
        // a node that never laid this text out: the widths it was last shaped at, e.g. by a measure (`Text::recent_widths`)
        let recent=t.recent_widths(natural).into_iter().filter(|w|*w>=width)
            .map(|w|pxui_text::layout_key(style,w,fingerprint,&[]));
        for key in last.into_iter().chain(Some(natural)).chain(recent) {
            if let Some(metrics)=t.reusable_layout(key,value,style,width) {
                if keep {memo.set(Some((key,fingerprint)));}
                return Some((key,metrics));
            }
        }
    }
    let key=pxui_text::layout_key(style,width,fingerprint,spans);
    // Text bytes changed: move the previous entry onto the new content key so shaping reuses the Buffer.
    let from=previous.filter(|(_,fp)|*fp!=fingerprint)
        .map(|(old,_)|old)
        .filter(|old|keep && *old!=key && !t.has_layout(key));
    if ahead && t.defer(key,value,style,width,spans) { return Some((key,Metrics::default())); }
    let metrics=t.layout_spans_from(from,key,value,style,width,spans)?;
    if keep {memo.set(Some((key,fingerprint)));}
    Some((key,metrics))
}
