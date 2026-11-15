use pxui_text::{Text,Metrics,Style,TextSpan};
use pxui_types::{Flags,ui::FIT_TEXT_PIXEL};

// Reuse the layout's retained text for equality; no second string buffer or warm byte hashing.
pub(super) fn lay_out_node(t:&mut Text,memo:&std::cell::Cell<Option<(u64,u64)>>,flags:Flags,style:&Style,width:f32,value:&str,spans:&[TextSpan])->Option<(u64,Metrics)> {
    let fingerprint=memo.get().filter(|(key,_)|flags & FIT_TEXT_PIXEL!=0 && t.has_text(*key,value))
        .map_or_else(||pxui_text::text_hash(value),|(_,fingerprint)|fingerprint);
    let key=pxui_text::layout_key(style,width,fingerprint,spans);
    let metrics=t.layout_spans(key,value,style,width,spans)?;
    if flags & FIT_TEXT_PIXEL!=0 {memo.set(Some((key,fingerprint)));}
    Some((key,metrics))
}


#[cfg(test)]
mod tests {
    use super::*;
    use pxui_text::Style;

    #[test]
    fn retained_node_reuses_its_fingerprint_and_revalidates_stale_layouts() {
        let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
        let memo=std::cell::Cell::new(None);
        let style=Style::default();
        let (key,_)=lay_out_node(&mut text,&memo,FIT_TEXT_PIXEL,&style,0.0,"a label",&[]).unwrap();
        memo.set(Some((key,7)));
        let (next,_)=lay_out_node(&mut text,&memo,FIT_TEXT_PIXEL,&style,100.0,"a label",&[]).unwrap();
        assert_eq!(next,pxui_text::layout_key(&style,100.0,7,&[]),"unchanged text must reuse the retained fingerprint, including across widths");
        text.layout(next,"a different label",&style,100.0);
        let (fresh,_)=lay_out_node(&mut text,&memo,FIT_TEXT_PIXEL,&style,100.0,"a label",&[]).unwrap();
        assert_eq!(fresh,pxui_text::content_key(&style,100.0,"a label",&[]),"a stale layout must revalidate its text");
    }
}
