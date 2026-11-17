mod string_pool {
    include!("../src/ui_core/string_pool.rs");

    #[test]
    fn indexed_string_donors_match_full_search_across_clear_growth_swap_and_frame_changes() {
        let mut slots:Vec<[String;3]>=(0..64).map(|i| {
            std::array::from_fn(|field| {
                let mut buffer=String::with_capacity([0,8,11,14,16,31,63,128][(i+field)%8]);
                if i%3==0 {buffer.push_str("live café");}
                buffer
            })
        }).collect();
        let mut pool=EmptyStrings::default();
        pool.reserve(slots.capacity());
        assert!(!pool.active());
        let mut random=1u64;
        for step in 0..1000 {
            if step%200==199 {
                slots.push(std::array::from_fn(|_|String::new()));
                pool.reserve(slots.capacity());
            }
            random=random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let own=(random>>32) as usize%slots.len();
            let field=Field::ALL[step%3];
            let text=["","x","café","small label","a moderately sized label","a much longer label that needs the retained large storage"][step%6];
            if step%11==0 {
                pool.begin_frame();assert!(!pool.active());
                pool.used(field,own,slots[own][field as usize].capacity());
                assert!(!pool.active(),"the stable-buffer path must not construct an index");
            }
            let cleared=(own+7)%slots.len();
            for field in Field::ALL {
                slots[cleared][field as usize].clear();
                pool.empty(field,cleared,slots[cleared][field as usize].capacity());
            }
            let capacity=slots[own][field as usize].capacity();
            let expected=slots.iter().enumerate().filter_map(|(i,slot)| {
                let buffer=&slot[field as usize];
                (i!=own && buffer.is_empty() && buffer.capacity()>=text.len() && (capacity<text.len() || buffer.capacity()<capacity)).then_some((i,buffer.capacity()))
            }).min_by_key(|entry|entry.1).map(|entry|entry.0);
            mem_sys::local_reset();
            if text.is_empty() {pool.empty(field,own,capacity);}
            else {
                if let Some(index)=pool.rebuild(field) {
                    index.extend(slots.iter().enumerate().filter_map(|(slot,value)| {
                        let buffer=&value[field as usize];
                        (buffer.is_empty() && buffer.capacity()!=0).then_some((buffer.capacity(),slot))
                    }));
                    index.sort_unstable();
                }
                let donor=pool.donor(field,own,capacity,text.len());
                assert!(pool.rebuild(field).is_none(),"one donor query builds each field index for the frame");
                assert_eq!(donor,expected,"step {step}, field {}",field as usize);
                if let Some(donor)=donor {
                    let buffer=std::mem::take(&mut slots[donor][field as usize]);
                    let mut old=std::mem::replace(&mut slots[own][field as usize],buffer);
                    old.clear();
                    slots[donor][field as usize]=old;
                    pool.empty(field,donor,capacity);
                }
            }
            assert_eq!(mem_sys::local_alloc_count(),0,"index construction, updates and donor transfers reuse storage");
            // A genuinely new string size can grow storage; index maintenance itself must not.
            let buffer=&mut slots[own][field as usize];
            buffer.clear();buffer.push_str(text);
            for field in Field::ALL {
                let index=&pool.indices[field as usize];
                assert!(index.len()<=slots.len() && index.capacity()>=slots.len());
                assert!(index.capacity()<=slots.capacity()*2,"metadata remains bounded by retained slot capacity");
                if pool.active & (1 << field as usize)!=0 {
                    for &(capacity,slot) in index {
                        assert!(slots[slot][field as usize].is_empty());
                        assert_eq!(slots[slot][field as usize].capacity(),capacity);
                    }
                    assert!(index.windows(2).all(|w|w[0]<w[1]),"each empty buffer has one sorted entry");
                }
            }
        }
    }
}

mod text_identity {
    include!("../src/ui_core/text_identity.rs");
}

#[cfg(test)]
mod text_identity_cases {
    use super::text_identity::lay_out_node;
    use pxui_text::{Style, Text};
    use pxui_types::Flags;

    #[test]
    fn retained_node_reuses_its_fingerprint_and_revalidates_stale_layouts() {
        let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
        let memo=std::cell::Cell::new(None);
        let style=Style::default();
        let (key,_)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,0.0,"a label",&[],false).unwrap();
        memo.set(Some((key,7)));
        // narrower than the natural layout (a wider width would show the natural one, `Text::reusable_layout`)
        let (next,_)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,10.0,"a label",&[],false).unwrap();
        assert_eq!(next,pxui_text::layout_key(&style,10.0,7,&[]),"unchanged text must reuse the retained fingerprint, including across widths");
        text.layout(next,"a different label",&style,10.0);
        let (fresh,_)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,10.0,"a label",&[],false).unwrap();
        assert_eq!(fresh,pxui_text::content_key(&style,10.0,"a label",&[]),"a stale layout must revalidate its text");
    }

    #[test]
    fn a_node_served_by_a_reusable_layout_keeps_its_fingerprint() {
        let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
        let style=Style::default();
        let natural=pxui_text::content_key(&style,0.0,"a label",&[]);
        text.layout(natural,"a label",&style,0.0);
        let memo=std::cell::Cell::new(None);
        let (key,_)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,500.0,"a label",&[],false).unwrap();
        assert_eq!(key,natural,"a wider width is served by the natural layout");
        assert_eq!(memo.get(),Some((natural,pxui_text::text_hash("a label"))),"the node remembers the natural key and fingerprint");
        // the next frame takes the fingerprint from the memo instead of hashing the bytes: a marker one stays in the memo
        memo.set(Some((natural,7)));
        let (again,_)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,500.0,"a label",&[],false).unwrap();
        assert_eq!((again,memo.get()),(natural,Some((natural,7))),"the memo's fingerprint was used, not a fresh hash");
    }

    #[test]
    fn a_draw_at_the_measured_width_is_served_by_the_measure_layout_without_shaping() {
        let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
        let style=Style { wrap: pxui_text::Wrap::Word, ..Style::default() };
        let memo=std::cell::Cell::new(None);
        let (key,m)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,40.0,"1234567 plays",&[],false).unwrap();
        assert!(m.lines>1 && m.w<40.0);
        let layouts=text.layout_count();
        let (drawn,dm)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,m.w,"1234567 plays",&[],false).unwrap();
        assert_eq!((drawn,dm,text.layout_count()),(key,m,layouts));
        assert_eq!(memo.get().map(|x|x.0),Some(key),"the memo keeps the layout");
        let (own,_)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,m.w-1.0,"1234567 plays",&[],false).unwrap();
        assert_ne!(own,key);
        assert_eq!(text.layout_count(),layouts+1);
    }

    #[test]
    fn a_new_node_drawing_text_measured_elsewhere_hits_the_measure_layout_without_shaping() {
        let glyphs=|text:&mut Text,key:u64|{
            let mut rects=Vec::with_capacity(64);
            text.glyphs(key,(3.0,5.0),1.0,&mut |g|rects.push(g.rect));
            rects
        };
        let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
        let style=Style { wrap: pxui_text::Wrap::Word, ..Style::default() };
        let (value,w)=("1234567 plays",40.0);
        // measured as UiText::measure does: no node, no memo
        let key=pxui_text::content_key(&style,w,value,&[]);
        let m=text.layout(key,value,&style,w);
        assert!(m.lines>1 && m.w<w);
        let mut alone=Text::new(&[pxui_text::PRAGMASEVKA]);
        let at=pxui_text::content_key(&style,m.w,value,&[]);
        alone.layout(at,value,&style,m.w);
        let reference=glyphs(&mut alone,at);
        let layouts=text.layout_count();
        let memo=std::cell::Cell::new(None);
        mem_sys::local_reset();
        let (hit,hm)=lay_out_node(&mut text,&memo,Flags::FIT_TEXT_PIXEL,&style,m.w,value,&[],false).unwrap();
        assert_eq!(mem_sys::local_alloc_count(),0);
        assert_eq!((hit,hm,text.layout_count()),(key,m,layouts),"a fresh node at the measured width shapes nothing");
        assert_eq!(glyphs(&mut text,hit),reference,"identical glyphs");
        let fresh=std::cell::Cell::new(None);
        let (own,_)=lay_out_node(&mut text,&fresh,Flags::FIT_TEXT_PIXEL,&style,m.w-1.0,value,&[],false).unwrap();
        assert_ne!(own,key);
        assert_eq!(text.layout_count(),layouts+1,"a narrower draw lays out its own");
    }
}
