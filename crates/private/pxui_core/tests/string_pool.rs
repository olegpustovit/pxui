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
